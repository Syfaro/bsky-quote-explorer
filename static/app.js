async function main() {
  const resp = await fetch(
    "/generic?uri=at://did:plc:rzo5xkl7skyv45npuzd2vv5u/app.bsky.feed.post/3ktt5v6m65c2y"
  );
  const data = await resp.json();

  const colorQuery = window.matchMedia("(prefers-color-scheme: dark)");

  const elements = {
    nodes: data.nodes.map((node) => {
      const name = node["also_known_as"].replace("at://", "");
      return { data: { name, ...node } };
    }),
    edges: data.edges.map((edge) => {
      const color = edge["edge_type"] === "quote" ? "#0074D9" : "#2ECC40";
      delete edge["id"];
      return { data: { color, ...edge } };
    }),
  };

  const cy = cytoscape({
    container: document.getElementById("viewer"),
    elements,
    style: [
      {
        selector: "node",
        style: {
          label: "data(name)",
          color: colorQuery.matches ? "white" : "black",
        },
      },
      {
        selector: "edge",
        style: {
          "line-color": "data(color)",
        },
      },
    ],
    layout: {
      name: "dagre",
      nodeDimensionsIncludeLabels: true,
    },
  });

  cy.autounselectify(true);

  cy.on("tap", "node", function () {
    const uri = this.data("uri");
    const parts = uri.split("/");
    const bsky = `https://bsky.app/profile/${parts[2]}/post/${parts[4]}`;
    window.open(bsky, "_blank");
  });

  addEventListener("resize", () => {
    cy.resize();
    cy.fit();
  });

  colorQuery.addEventListener("change", () => {
    const color = colorQuery.matches ? "white" : "black";
    cy.nodes().style("color", color);
  });

  let enabledCollections = new Set(["quote", "reply"]);

  function filterData() {
    cy.remove("");

    const nodes = elements.nodes.map((node) => {
      return { group: "nodes", ...node };
    });

    cy.add(nodes);

    const edges = elements.edges
      .filter((edge) => {
        return enabledCollections.has(edge.data["edge_type"]);
      })
      .map((edge) => {
        return { group: "edges", ...edge };
      });

    cy.add(edges);

    cy.nodes((el) => {
      if (el.isNode() && el.degree() === 0) {
        cy.remove(el);
      }
    });

    cy.layout({
      name: "dagre",
      nodeDimensionsIncludeLabels: true,
    }).run();
  }

  Array.from(document.querySelectorAll("input.edge-filter")).forEach((elem) => {
    elem.addEventListener("change", (ev) => {
      const selection = ev.target.value;
      if (ev.target.checked) {
        enabledCollections.add(selection);
      } else {
        enabledCollections.delete(selection);
      }
      filterData();
    });
  });
}

main().then(() => console.debug("Done!"));
