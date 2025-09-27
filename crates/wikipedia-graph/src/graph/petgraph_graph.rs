use crate::WikipediaPage;

use super::WikipediaGraph;

use petgraph::graph::{IndexType, NodeIndex};
use petgraph::stable_graph::StableDiGraph;

impl<Index: IndexType> WikipediaGraph<NodeIndex<Index>>
    for StableDiGraph<WikipediaPage, (), Index>
{
    fn add_node(&mut self, page: WikipediaPage) -> NodeIndex<Index> {
        self.add_node(page)
    }

    fn add_edge(&mut self, from: NodeIndex<Index>, to: NodeIndex<Index>) {
        self.add_edge(from, to, ());
    }

    fn node_weight(&self, index: NodeIndex<Index>) -> Option<&WikipediaPage> {
        self.node_weight(index)
    }

    fn node_weights(&self) -> Vec<&WikipediaPage> {
        self.node_weights().collect()
    }

    fn node_weight_mut(&mut self, index: NodeIndex<Index>) -> Option<&mut WikipediaPage> {
        self.node_weight_mut(index)
    }

    fn node_indicies(&self) -> Vec<(&WikipediaPage, NodeIndex<Index>)> {
        self.node_weights().zip(self.node_indices()).collect()
    }

    fn edge_exists(&self, lhs: NodeIndex<Index>, rhs: NodeIndex<Index>) -> bool {
        self.contains_edge(lhs, rhs)
    }
}
