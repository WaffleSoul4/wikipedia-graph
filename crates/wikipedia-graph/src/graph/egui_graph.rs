use crate::WikipediaPage;
use egui_graphs::Graph;

use super::WikipediaGraph;

use petgraph::{
    Directed,
    graph::{IndexType, NodeIndex},
};

impl<Index: IndexType> WikipediaGraph<NodeIndex<Index>>
    for Graph<WikipediaPage, (), Directed, Index>
{
    fn add_node(&mut self, page: WikipediaPage) -> NodeIndex<Index> {
        self.add_node(page)
    }

    fn add_edge(&mut self, from: NodeIndex<Index>, to: NodeIndex<Index>) {
        // I hate when it's like "edge #21342353232"
        self.add_edge_with_label(from, to, (), String::new());
    }

    fn node_weight(&self, index: NodeIndex<Index>) -> Option<&WikipediaPage> {
        Some(self.node(index)?.payload())
    }

    fn node_weights(&self) -> Vec<&WikipediaPage> {
        self.nodes_iter().map(|node| node.1.payload()).collect()
    }

    fn node_weight_mut(&mut self, index: NodeIndex<Index>) -> Option<&mut WikipediaPage> {
        Some(self.node_mut(index)?.payload_mut())
    }

    fn node_indicies(&self) -> Vec<(&WikipediaPage, NodeIndex<Index>)> {
        self.nodes_iter()
            .map(|(index, node)| (node.payload(), index))
            .collect()
    }

    fn edge_exists(&self, lhs: NodeIndex<Index>, rhs: NodeIndex<Index>) -> bool {
        self.edges_connecting(lhs, rhs).next().is_some()
    }
}
