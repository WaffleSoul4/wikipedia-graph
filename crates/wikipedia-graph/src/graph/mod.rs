use crate::WikipediaPage;

#[cfg(feature = "petgraph")]
mod petgraph_graph;

#[cfg(feature = "egui_graphs")]
mod egui_graph;

/// The type used for indexing nodes on the graph
///
///  *This alias requires the `graphs` feature*
pub type DefaultIndexType = usize;

/// A trait that adds methods for manipulating and expanding wikipedia pages
///
///  *This trait requires the `graphs` feature*
pub trait WikipediaGraph<IndexType: Clone> {
    /// Add a node to the graph
    ///
    ///  *This method requires the `graphs` feature*
    fn add_node(&mut self, page: WikipediaPage) -> IndexType;

    /// Add an edge to the graph
    ///
    ///  *This method requires the `graphs` feature*
    fn add_edge(&mut self, from: IndexType, to: IndexType);

    /// Get the weight of a node on the graph, or None if it doesn't exist
    ///
    ///  *This method requires the `graphs` feature*
    fn node_weight(&self, index: IndexType) -> Option<&WikipediaPage>;

    /// Get all of the node weights on the graph
    ///
    ///  *This method requires the `graphs` feature*
    fn node_weights(&self) -> Vec<&WikipediaPage>;

    /// Get all of the node indicies and their weights
    ///
    ///  *This method requires the `graphs` feature*
    fn node_indicies(&self) -> Vec<(&WikipediaPage, IndexType)>;

    /// Get the weight of a node on the graph mutably, or None if it doesn't exist
    ///
    ///  *This method requires the `graphs` feature*
    fn node_weight_mut(&mut self, index: IndexType) -> Option<&mut WikipediaPage>;

    /// Check if and edge exists
    ///
    ///  *This method requires the `graphs` feature*
    fn edge_exists(&self, lhs: IndexType, rhs: IndexType) -> bool;

    /// Get a list of all nodes with their weights and indicies cloned
    ///
    ///  *This method requires the `graphs` feature*
    fn node_indicies_owned(&self) -> Vec<(WikipediaPage, IndexType)> {
        self.node_indicies()
            .into_iter()
            .map(|(page, index)| (page.clone(), index))
            .collect()
    }

    /// Place all linked pages as nodes on the graph and return only newly created nodes
    ///
    /// *This method requires the `graphs` feature*
    #[cfg(feature = "client")]
    fn try_expand_node(&mut self, index: IndexType) -> Option<Vec<IndexType>> {
        let page = self.node_weight_mut(index.clone())?.clone();

        let linked_pages = page.try_get_linked_pages()?;

        let mut indicies = Vec::new();

        for page in linked_pages.into_iter() {
            match self.node_exists_with_value(&page) {
                Some(existing_index) => {
                    if !self.edge_exists(index.clone(), existing_index.clone()) {
                        self.add_edge(index.clone(), existing_index);
                    }
                }
                None => indicies.push(self.add_node(page)),
            }
        }

        indicies.iter().for_each(|node_index| {
            self.add_edge(index.clone(), node_index.clone());
        });

        Some(indicies)
    }

    /// Check if a node exists with a specified value
    ///
    ///  *This method requires the `graphs` feature*
    fn node_exists_with_value(&self, page: &WikipediaPage) -> Option<IndexType> {
        self.node_indicies()
            .iter()
            .find(|(node_page, _)| page.pathinfo() == node_page.pathinfo())
            .map(|(_, index)| index.clone())
    }
}
