use std::alloc::Layout;

use egui::{
    Color32, Context, DragValue, Frame, Pos2, RichText, ScrollArea, Sense, Slider, TextEdit, Ui,
    UiBuilder,
};
use egui::{Key, Rect, Spinner, Vec2};
use egui_graphs::Metadata;
use log::{error, warn};
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::EdgeRef;
use wikipedia_graph::{WikipediaClient, WikipediaGraph, WikipediaPage};

use crate::{InternetStatus, WikipediaGraphApp};

impl WikipediaGraphApp {
    pub fn search_bar(&mut self, ctx: &Context) {
        egui::Window::new("Node Search").show(ctx, |ui| {
            ui.add(
                TextEdit::singleline(&mut self.search_data.query).hint_text("Search added nodes"),
            );

            if !self.search_data.query.is_empty() {
                let indices = self.graph.node_indicies();

                let pages = self.search_data.search_n_pages(indices, 10);

                for (name, index) in pages {
                    ui.scope(|ui| {
                        let visuals = ui.visuals();

                        let fill = if !ui.rect_contains_pointer(ui.max_rect()) {
                            Color32::TRANSPARENT
                        } else {
                            visuals.code_bg_color
                        };

                        Frame::NONE.corner_radius(3.).inner_margin(2.).outer_margin(2.).fill(fill).show(ui, |ui| {
                            let label = ui.label(name);

                            if label.clicked() {
                                self.selected_node = Some(index)
                            }
                        });
                    });
                }
            };
        });
    }

    pub fn keybinds(&mut self, ui: &mut Ui) {
        self.control_settings.movement.x = match (
            ui.input(|input| input.key_pressed(Key::A)),
            ui.input(|input| input.key_pressed(Key::D)),
        ) {
            (true, false) => 40.0,
            (false, true) => -40.0,
            _ => 0.0,
        };

        self.control_settings.movement.y = match (
            ui.input(|input| input.key_pressed(Key::W)),
            ui.input(|input| input.key_pressed(Key::S)),
        ) {
            (true, false) => 40.0,
            (false, true) => -40.0,
            _ => 0.0,
        };

        let mut meta = Metadata::load(ui);

        if ui.input(|input| input.key_pressed(Key::Space)) {
            let center = meta.screen_to_canvas_pos(ui.min_rect().center());

            if meta.zoom > 1.0 {
                meta.zoom = 0.1;
            } else {
                meta.zoom = 3.0;
            }

            self.focus_point_from_meta(ui, &mut meta, center.to_vec2());
        }

        meta.save(ui);
    }

    pub fn layout_settings(&mut self, ui: &mut Ui) {
        let layout_settings = &mut self.layout_settings;

        ui.add(Slider::new(&mut layout_settings.c_attract, 0.0..=10.0).text("Attraction"));
        ui.add(Slider::new(&mut layout_settings.c_repulse, 0.0..=10.0).text("Repulsion"));
        ui.add(Slider::new(&mut layout_settings.damping, 0.0..=10.0).text("Damping"));
        ui.add(Slider::new(&mut layout_settings.epsilon, 0.0..=10.0).text("Epsilon"));
        ui.add(Slider::new(&mut layout_settings.k_scale, 0.0..=10.0).text("Scale"));
        ui.add(Slider::new(&mut layout_settings.dt, 0.0..=10.0).text("dt"));
        ui.add(Slider::new(&mut layout_settings.max_step, 0.0..=50.0).text("Max step"));
        ui.checkbox(&mut layout_settings.is_running, "is running");
    }

    pub fn control_settings(&mut self, ui: &mut Ui) {
        ui.checkbox(
            &mut self.control_settings.focus_selected,
            "Focus selected node",
        );

        let mut meta = Metadata::load(ui);

        ui.horizontal(|ui| {
            ui.label("Zoom:");
            ui.add(
                Slider::new(&mut meta.zoom, 100.0..=0.005)
                    .logarithmic(true)
                    .custom_formatter(|zoom, _| format!("{zoom:.2}")),
            );
        });

        if self.control_settings.focus_selected {
            ui.disable();
        }

        ui.checkbox(&mut self.control_settings.key_input, "Keyboard Input");

        ui.collapsing("Pan", |ui| {
            ui.horizontal(|ui| {
                ui.label("x:");
                ui.add(DragValue::new(&mut meta.pan.x).speed(300))
            });

            ui.horizontal(|ui| {
                ui.label("y:");
                ui.add(DragValue::new(&mut meta.pan.y).speed(300))
            });
        });

        meta.save(ui);
    }

    pub fn style_settings(&mut self, ui: &mut Ui) {
        let style_settings = &mut self.style_settings;

        ui.checkbox(&mut style_settings.labels, "Show labels");
    }

    pub fn random_controls(&mut self, ui: &mut Ui) {
        if ui.button("Select random node").clicked() {
            self.select_random();
        }

        if ui.button("Expand random node").clicked() {
            self.expand_random();
        }
    }

    pub fn node_editor(&mut self, ui: &mut Ui) {
        let node_editor = &mut self.node_editor;

        if ui.button("Clear all nodes").clicked() {
            self.graph.g_mut().clear();

            self.selected_node = None;
        }

        ui.add(
            TextEdit::singleline(&mut node_editor.page_title).hint_text("Enter page title here"),
        );

        if ui.button("Create/Select node").clicked() {
            let page = WikipediaPage::from_title(&node_editor.page_title);
            let index = if let Some(index) = <egui_graphs::Graph<WikipediaPage> as WikipediaGraph<
                NodeIndex,
            >>::node_exists_with_value(
                &self.graph, &page
            ) {
                index
            } else {
                let index = self.graph.add_node(page);

                let page = self.graph.node_mut(index).unwrap();

                match page.payload_mut().load_page_text(&self.client) {
                    Ok(_) => {
                        page.set_label(page.payload().title());
                    }
                    Err(e) => {
                        let payload = page.payload().clone();
                        error!("Request for {} failed: {e}", self.url_of_page(&payload))
                    }
                };

                index
            };

            self.selected_node = Some(index);
        }
    }

    pub fn perf(&mut self, ui: &mut Ui) {
        let frame_counter = &mut self.frame_counter;

        ui.label(format!("Fps: {}", frame_counter.fps));
    }

    pub fn node_position_ui(&mut self, ui: &mut Ui, index: NodeIndex) {
        match self.graph.node_mut(index) {
            Some(node) => {
                let mut pos = node.location().clone();

                ui.collapsing("Position", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("x:");
                        ui.add(DragValue::new(&mut pos.x).speed(400))
                    });

                    ui.horizontal(|ui| {
                        ui.label("y:");
                        ui.add(DragValue::new(&mut pos.y).speed(400))
                    });
                });

                node.set_location(pos);
            }
            None => warn!("Selected node does not exist"),
        };
    }

    pub fn node_details_ui(&mut self, ui: &mut Ui, index: NodeIndex) {
        match self.graph.node_mut(index) {
            Some(node) => {
                let page = node.payload_mut();

                let title = page.title();

                let pathinfo = page.pathinfo().clone();

                let page_text_loaded = page.is_page_text_loaded();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label(RichText::new(title).size(30.0));

                    ui.hyperlink_to(
                        "Wikipedia Page",
                        self.url_of(index).expect("Selected node doesn't exist"),
                    );

                    if ui.button("Expand node").clicked() {
                        self.expand_node(index);
                    }

                    if ui.button("Remove node").clicked() {
                        self.remove_selected();
                    }

                    self.node_position_ui(ui, index);

                    ui.collapsing("Struct data", |ui| {
                        ui.label(format!("pathinfo: {}", pathinfo));
                        ui.label(format!("page text loaded: {}", page_text_loaded));
                    });

                    ui.separator();

                    ui.collapsing("Outgoing Nodes", |ui| {
                        self.connected_nodes_ui(ui, index, petgraph::Direction::Outgoing);
                    });

                    ui.collapsing("Incoming Nodes", |ui| {
                        self.connected_nodes_ui(ui, index, petgraph::Direction::Incoming);
                    });

                    let button = ui
                        .button("Expand all connected")
                        .on_hover_text("You must sacrifice a single cpu core to click this button");

                    if button.clicked() {
                        self.expand_connected_nodes(index);
                    };
                });
            }
            None => warn!("Selected node does not exist"),
        };
    }

    pub fn connected_nodes<'a>(
        graph: &'a egui_graphs::Graph<WikipediaPage>,
        index: NodeIndex,
        direction: petgraph::EdgeDirection,
    ) -> impl Iterator<Item = NodeIndex> + 'a {
        graph
            .edges_directed(index, direction)
            .map(|edge_reference| edge_reference.id())
            .flat_map(move |edge_index| {
                let connected_node =
                    graph
                        .edge_endpoints(edge_index)
                        .map(|(source, target)| match direction {
                            petgraph::Direction::Outgoing => target,
                            petgraph::Direction::Incoming => source,
                        });

                if connected_node.is_none() {
                    error!("Failed to locate a connected edge of the selected node");
                }

                connected_node
            })
    }

    pub fn connected_nodes_ui(
        &mut self,
        ui: &mut Ui,
        index: NodeIndex,
        direction: petgraph::EdgeDirection,
    ) {
        let _ = Self::connected_nodes(&self.graph, index, direction)
            .flat_map(|connected_index| {
                let node_data = self
                    .graph
                    .node(connected_index)
                    .map(|node| (node.label(), connected_index));

                if node_data.is_none() {
                    error!("Failed to locate a connected node of the selected node");
                }

                node_data
            })
            .for_each(|(label, connected_index)| {
                ui.collapsing(label, |ui| {
                    if ui.button("Select node").clicked() {
                        self.selected_node = Some(connected_index)
                    }
                });
            });
    }

    pub fn internet_unavailable_ui(ui: &mut Ui, remaining_seconds: f32, error: String) -> bool {
        let center = ui.max_rect().center();

        let max_rect = Rect::from_center_size(center, Vec2::new(260.0, 75.0));

        let mut clicked: bool = false;

        ui.put(max_rect, |ui: &mut Ui| {
            ui.add(Spinner::new().size(50.0));

            ui.label(format!(
                "Internet Unavailable, trying again in {:.0} seconds",
                remaining_seconds
            ));

            ui.label(error);

            let button = ui.button("Test Now");

            clicked = button.clicked();

            button
        });

        clicked
    }
}
