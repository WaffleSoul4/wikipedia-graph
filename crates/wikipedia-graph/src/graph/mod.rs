use crate::WikipediaPage;

#[cfg(feature = "client")]
use crate::client::WikipediaClient;

#[cfg(feature = "petgraph")]
mod petgraph_graph;

#[cfg(feature = "egui_graphs")]
mod egui_graph;

pub trait Indexable: Copy {
    fn index(&self) -> usize;
    fn from_index(index: usize) -> Self;
}

#[cfg(any(feature = "petgraph", feature = "egui_graphs"))]
impl<T: petgraph::graph::IndexType> Indexable for T {
    fn index(&self) -> usize {
        self.index()
    }

    fn from_index(index: usize) -> Self {
        Self::new(index)
    }
}

pub trait WikipediaGraph<NodeIndex: Copy + Indexable> {
    fn add_node(&mut self, page: WikipediaPage) -> NodeIndex;

    fn add_edge(&mut self, from: NodeIndex, to: NodeIndex);

    fn node_weight(&self, index: NodeIndex) -> Option<&WikipediaPage>;

    fn node_weights(&self) -> Vec<&WikipediaPage>;

    fn node_indicies(&self) -> Vec<(&WikipediaPage, NodeIndex)>;

    fn node_weight_mut(&mut self, index: NodeIndex) -> Option<&mut WikipediaPage>;

    fn edge_exists(&self, lhs: NodeIndex, rhs: NodeIndex) -> bool;

    fn node_indicies_owned(&self) -> Vec<(WikipediaPage, NodeIndex)> {
        self.node_indicies()
            .into_iter()
            .map(|(page, index)| (page.clone(), index))
            .collect()
    }

    /// Place all linked pages as nodes on the graph. Returns a vector of only newly created nodes.
    #[cfg(feature = "client")]
    fn try_expand_node(
        &mut self,
        index: NodeIndex,
        client: &WikipediaClient,
    ) -> Result<Option<Vec<NodeIndex>>, crate::client::HttpError> {
        let page = self.node_weight_mut(index);

        let page = match page {
            Some(t) => t,
            None => return Ok(None),
        };

        let linked_pages = page
            .load_page_text(client)?
            .try_get_linked_pages()
            .expect("Pages failed to load");

        let node_indicies = linked_pages
            .into_iter()
            .filter_map(|page| match self.node_exists(&page) {
                Some(existing_index) => {
                    if !self.edge_exists(index, existing_index) {
                        self.add_edge(index, existing_index);
                    }

                    None
                }
                None => Some(self.add_node(page)),
            })
            .collect::<Vec<NodeIndex>>();

        node_indicies.iter().for_each(|node_index| {
            self.add_edge(index, node_index.clone());
        });

        Ok(Some(node_indicies))
    }

    fn node_exists(&self, page: &WikipediaPage) -> Option<NodeIndex> {
        self.node_indicies()
            .iter()
            .find(|(node_page, _)| {
                page.pathinfo() == node_page.pathinfo()
                    || page
                        .try_get_title()
                        .map(|result| result.unwrap_or("FAILED".to_string()))
                        == node_page
                            .try_get_title()
                            .map(|result| result.unwrap_or("FAILED".to_string()))
            })
            .map(|(_, index)| *index)
    }
}
