#[path = "common.rs"]
mod common;

#[cfg(feature = "petgraph")]
mod petgraph {
    use crate::common::{self, multekrem_page};
    use petgraph::prelude::StableDiGraph;
    use wikipedia_graph::{WikipediaClient, WikipediaGraph, WikipediaPage};

    #[test]
    fn expand_nodes() {
        let mut graph: StableDiGraph<WikipediaPage, ()> =
            petgraph::stable_graph::StableDiGraph::default();

        let multekrem_index = graph.add_node(multekrem_page());

        let connected = graph
            .try_expand_node(multekrem_index, &WikipediaClient::default())
            .expect("")
            .expect("Multekrem node does not exist");

        connected
            .iter()
            .map(|idx| {
                graph
                    .node_weight(idx.clone())
                    .expect("Page expansion returned and invalid index")
            })
            .zip(common::multekrem_pages_iter())
            .for_each(|(known, node)| assert_eq!(known.pathinfo(), node.pathinfo()));
    }

    #[test]
    fn double_expand_nodes() {
        let client = WikipediaClient::default();

        let mut graph_1: StableDiGraph<WikipediaPage, ()> =
            petgraph::stable_graph::StableDiGraph::default();

        let multekrem_index = graph_1.add_node(multekrem_page());

        let connected_1 = graph_1
            .try_expand_node(multekrem_index, &client)
            .expect("If this happens, just use a different crate")
            .expect("Multekrem node does not exist");

        let mut graph_2 = graph_1.clone();

        let connected_2 = graph_2
            .try_expand_node(multekrem_index, &client)
            .expect("If this happens, just use a different crate")
            .expect("Multekrem node does not exist");

        assert_eq!(connected_1, connected_2);

        assert!(graph_1.edge_weights().eq(graph_2.edge_weights()));
        assert!(
            graph_1
                .node_weights()
                .map(|weight| weight.pathinfo())
                .eq(graph_2.node_weights().map(|wieght| wieght.pathinfo()))
        )
    }
}
