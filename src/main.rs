use std::{
    collections::{HashMap, HashSet},
    fmt::Write,
    net::SocketAddr,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

use async_nats::{jetstream::consumer, ServerAddr};
use atrium_api::{
    app::bsky::feed::post::{Record as Post, RecordEmbedRefs},
    types::Union,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json, Router,
};
use clap::Parser;
use futures::StreamExt;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing::instrument;

#[derive(Parser)]
struct Config {
    #[clap(long, env("DATABASE_URL"))]
    pub database_url: String,

    #[clap(
        long,
        env("NATS_HOST"),
        use_value_delimiter = true,
        value_delimiter = ','
    )]
    pub nats_host: Vec<ServerAddr>,
    #[clap(long, env("NATS_NKEY"))]
    pub nats_nkey: Option<String>,

    #[clap(long, env, default_value = "0.0.0.0:8080")]
    pub bind_addr: SocketAddr,

    #[clap(long, env)]
    pub root_uri: String,
}

#[derive(Deserialize)]
struct MessageData {
    repo: String,
    path: String,
    #[serde(rename = "data")]
    post: Post,
}

struct AppState {
    conn: PgPool,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt::init();
    let config = Config::parse();

    let client = reqwest::Client::default();

    let pool = PgPool::connect(&config.database_url).await?;
    sqlx::migrate!().run(&pool).await?;

    let nats = connect_nats(&config).await?;

    let js = async_nats::jetstream::new(nats);
    let resolver = Arc::new(DidResolver::new(pool.clone(), client.clone()));

    tokio::spawn(start_thread(pool.clone(), resolver, js, config.root_uri));

    let app_state = Arc::new(AppState { conn: pool });

    let app = Router::new()
        .route("/generic", axum::routing::get(generic))
        .route("/graphviz", axum::routing::get(graphviz))
        .fallback_service(ServeDir::new("static"))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[derive(Deserialize)]
struct GraphQuery {
    uri: String,
}

#[derive(Serialize)]
struct NodeInfo {
    id: i64,
    uri: String,
    did: String,
    also_known_as: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    text: String,
}

#[derive(Serialize)]
struct EdgeInfo {
    source: i64,
    target: i64,
}

#[derive(Serialize)]
struct GraphResponse {
    nodes: Vec<NodeInfo>,
    edges: Vec<EdgeInfo>,
}

async fn collect_info(conn: &PgPool, uri: &str) -> eyre::Result<GraphResponse> {
    let root_id = sqlx::query_file_scalar!("queries/info/get_id.sql", uri)
        .fetch_one(conn)
        .await?;

    let nodes = sqlx::query_file!("queries/info/get_nodes.sql", root_id)
        .map(|row| NodeInfo {
            id: row.id,
            uri: row.uri,
            did: row.did,
            also_known_as: row.also_known_as,
            created_at: row.created_at,
            text: row.post_text,
        })
        .fetch_all(conn);
    let edges = sqlx::query_file!("queries/info/get_edges.sql", root_id)
        .map(|row| EdgeInfo {
            source: row.source_node_id,
            target: row.target_node_id,
        })
        .fetch_all(conn);

    let (nodes, edges) = tokio::try_join!(nodes, edges)?;

    Ok(GraphResponse { nodes, edges })
}

async fn generic(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GraphQuery>,
) -> Result<Json<GraphResponse>, StatusCode> {
    let resp = collect_info(&state.conn, &query.uri)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(resp))
}

async fn graphviz(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GraphQuery>,
) -> Result<String, StatusCode> {
    let resp = collect_info(&state.conn, &query.uri)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut output = String::new();
    output.push_str("digraph tree {\n");

    for node in resp.nodes {
        writeln!(
            output,
            "\t\"{}\" [label=<\n\t\t<font face=\"Sans-Serif\">{}</font><br/>\n\t\t<font face=\"Sans-Serif\" color=\"#37474F\">{}</font>\n\t>, shape=rectangle, fixedsize=true, width=2.7, height=0.75]",
            node.id,
            node.also_known_as
                .as_deref()
                .and_then(|aka| aka.strip_prefix("at://"))
                .unwrap_or(&node.did),
            node.created_at.format("%b %-d, %T")
        )
        .unwrap();
    }

    for edge in resp.edges {
        writeln!(output, "\t\"{}\" -> \"{}\"", edge.source, edge.target).unwrap();
    }

    output.push_str("}\n");

    Ok(output)
}

async fn connect_nats(config: &Config) -> eyre::Result<async_nats::Client> {
    let nats_opts = if let Some(nats_nkey) = &config.nats_nkey {
        async_nats::ConnectOptions::with_nkey(nats_nkey.clone())
    } else {
        async_nats::ConnectOptions::default()
    };

    let nats = nats_opts.connect(&config.nats_host).await?;

    Ok(nats)
}

async fn start_thread(
    pool: PgPool,
    resolver: Arc<DidResolver>,
    js: async_nats::jetstream::Context,
    root: String,
) -> eyre::Result<()> {
    let stream = js.get_stream("bsky-ingest").await?;
    let consumer = stream
        .create_consumer(consumer::pull::Config {
            filter_subject: "bsky.ingest.commit.create.app.bsky.feed.post".to_string(),
            ..Default::default()
        })
        .await?;

    let mut tree = DbTree::new(pool, resolver, root).await?;

    let mut messages = consumer.messages().await?;

    while let Some(Ok(msg)) = messages.next().await {
        msg.ack().await.map_err(|err| eyre::format_err!(err))?;

        let Ok(data) = serde_json::from_slice::<MessageData>(&msg.payload) else {
            tracing::warn!(
                "could not decode post: {}",
                String::from_utf8_lossy(&msg.payload)
            );
            continue;
        };

        tree.process(data).await?;
    }

    Ok(())
}

struct DidResolver {
    conn: PgPool,
    client: reqwest::Client,
    cache: Mutex<LruCache<String, Option<String>>>,
}

impl DidResolver {
    fn new(conn: PgPool, client: reqwest::Client) -> Self {
        Self {
            conn,
            client,
            cache: Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap())),
        }
    }

    #[instrument(err, skip(self))]
    async fn resolve_did(&self, did: &str) -> eyre::Result<Option<String>> {
        {
            tracing::trace!("checking lru cache for did");
            if let Some(also_known_as) = self.cache.lock().unwrap().get(did) {
                tracing::debug!(?also_known_as, "lru cache had did value");
                return Ok(also_known_as.clone());
            }
        }

        tracing::trace!("checking database for did");
        if let Some(also_known_as) = sqlx::query_file_scalar!("queries/did/lookup.sql", did)
            .fetch_optional(&self.conn)
            .await?
        {
            tracing::debug!(?also_known_as, "database had did value");
            return Ok(also_known_as);
        }

        let also_known_as = self.resolve_did_uncached(did).await;

        if let Ok(ref also_known_as) = also_known_as {
            tracing::debug!(?also_known_as, "successfully resolved did");

            {
                self.cache
                    .lock()
                    .unwrap()
                    .put(did.to_string(), also_known_as.clone());
            }

            sqlx::query_file!("queries/did/save.sql", did, also_known_as.as_deref())
                .execute(&self.conn)
                .await?;
        }

        also_known_as
    }

    #[instrument(err, skip_all)]
    async fn resolve_did_uncached(&self, did: &str) -> eyre::Result<Option<String>> {
        if let Some(_name) = did.strip_prefix("did:web:") {
            todo!("did web resolution");
        }

        let mut data: serde_json::Value = self
            .client
            .get(format!("https://plc.directory/{did}"))
            .send()
            .await?
            .json()
            .await?;

        let also_known_as = data["alsoKnownAs"]
            .as_array_mut()
            .and_then(|also_known_as| also_known_as.pop())
            .and_then(|also_known_as| also_known_as.as_str().map(ToString::to_string));

        Ok(also_known_as)
    }
}

struct DbTree {
    conn: PgPool,
    resolver: Arc<DidResolver>,
    root_id: i64,
    uris: HashSet<String>,
    known_nodes: HashMap<String, i64>,
}

impl DbTree {
    async fn new(conn: PgPool, resolver: Arc<DidResolver>, root: String) -> eyre::Result<Self> {
        let root_id = sqlx::query_file_scalar!("queries/tree/new.sql", &root)
            .fetch_one(&conn)
            .await?;

        tracing::debug!(root_id, "created thread root");

        let known_nodes: HashMap<_, _> = sqlx::query_file!("queries/tree/known_nodes.sql", root_id)
            .map(|row| (row.uri, row.id))
            .fetch_all(&conn)
            .await?
            .into_iter()
            .collect();

        tracing::debug!(known_nodes = known_nodes.len(), "loaded existing nodes");

        let uris = known_nodes.keys().cloned().chain([root]).collect();

        Ok(Self {
            conn,
            resolver,
            root_id,
            uris,
            known_nodes,
        })
    }

    #[instrument(err, skip_all)]
    async fn process(&mut self, data: MessageData) -> eyre::Result<bool> {
        let post_uri = format!("at://{}/{}", data.repo, data.path);
        if self.uris.contains(&post_uri) && !self.known_nodes.contains_key(&post_uri) {
            tracing::info!("known uri that isn't known node, adding");
            self.add_node(post_uri, &data.post).await?;

            return Ok(true);
        }

        let record = match &data.post.embed {
            Some(Union::Refs(RecordEmbedRefs::AppBskyEmbedRecordMain(main))) => {
                tracing::trace!("found embed record");
                &main.record
            }
            _ => {
                tracing::trace!("no embed record");
                return Ok(false);
            }
        };

        let embedding_uri = &record.uri;
        if !self.uris.contains(embedding_uri) {
            tracing::trace!(embedding_uri, "known uris does not contain");
            return Ok(false);
        }
        tracing::info!(embedding_uri, "found known uri");

        let source_node_id = *self
            .known_nodes
            .get(embedding_uri)
            .ok_or_else(|| eyre::eyre!("missing id for uri {embedding_uri}"))?;
        let target_node_id = self.add_node(post_uri, &data.post).await?;

        self.add_edge(source_node_id, target_node_id).await?;

        Ok(true)
    }

    #[instrument(err, skip(self, post))]
    async fn add_node(&mut self, uri: String, post: &Post) -> eyre::Result<i64> {
        if let Some(node_id) = self.known_nodes.get(&uri) {
            tracing::debug!(node_id, "already had id for node");
            return Ok(*node_id);
        }

        let Some(did) = uri.split('/').nth(2) else {
            eyre::bail!("post was missing did");
        };

        self.resolver.resolve_did(did).await?;

        let created_at: chrono::DateTime<chrono::Utc> = (*post.created_at.as_ref()).into();

        let node_id = sqlx::query_file_scalar!(
            "queries/tree/insert_thread_node.sql",
            self.root_id,
            uri,
            did,
            created_at,
            post.text
        )
        .fetch_one(&self.conn)
        .await?;

        self.known_nodes.insert(uri.clone(), node_id);
        self.uris.insert(uri);
        tracing::debug!(node_id, "inserted id for node");

        Ok(node_id)
    }

    #[instrument(err, skip(self))]
    async fn add_edge(&self, source_node_id: i64, target_node_id: i64) -> eyre::Result<()> {
        sqlx::query_file!(
            "queries/tree/insert_thread_edge.sql",
            self.root_id,
            source_node_id,
            target_node_id
        )
        .execute(&self.conn)
        .await?;

        Ok(())
    }
}
