pub mod builder;
mod ui;

use crossbeam::channel::{Receiver, Sender};
use eframe::{App, CreationContext};
use egui::{Context, Pos2, Ui, Vec2};
use egui_graphs::{
    FruchtermanReingoldWithCenterGravity, FruchtermanReingoldWithCenterGravityState, Graph,
    GraphView, LayoutForceDirected, Metadata, SettingsInteraction, SettingsNavigation,
    SettingsStyle, events::Event,
};
use fastrand::Rng;
use log::{error, info};
use petgraph::graph::NodeIndex;
use std::{
    cell::RefCell,
    rc::Rc,
};
use web_time::{Duration, Instant};

use wikipedia_graph::{HttpError, Language, Url, WikipediaClient, WikipediaGraph, WikipediaPage};

use crate::builder::WikipediaGraphAppBuilder;

pub struct WikipediaGraphApp {
    pub graph: Graph<WikipediaPage>,
    pub interaction_settings: SettingsInteraction,
    pub navigation_settings: SettingsNavigation,
    pub layout_settings: LayoutSettings,
    #[cfg(not(target_arch = "wasm32"))]
    pub event_writer: Sender<Event>,
    #[cfg(not(target_arch = "wasm32"))]
    pub event_reader: Receiver<Event>,
    #[cfg(target_arch = "wasm32")]
    pub event_buffer: Rc<RefCell<Vec<Event>>>,
    pub client: WikipediaClient,
    pub frame_counter: FrameCounter,
    pub selected_node: Option<NodeIndex>,
    pub control_settings: ControlSettings,
    pub rng: Rng,
    pub node_editor: NodeEditor,
    pub style_settings: StyleSettings,
    pub initialization: u8,
    pub internet_status: InternetStatus,
    pub language: Language,
}

pub struct FrameCounter {
    fps: f32,
    last_update: Instant,
}

impl Default for FrameCounter {
    fn default() -> Self {
        FrameCounter {
            fps: 0.0,
            last_update: Instant::now(),
        }
    }
}

impl FrameCounter {
    fn update_fps(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update);

        self.fps = 1.0 / elapsed.as_secs_f32();

        self.last_update = now;
    }
}

pub struct LayoutSettings {
    k_scale: f32,
    c_attract: f32,
    c_repulse: f32,
    epsilon: f32,
    damping: f32,
    dt: f32,
    max_step: f32,
    is_running: bool,
}

impl Default for LayoutSettings {
    fn default() -> Self {
        LayoutSettings {
            k_scale: 5.0,
            c_attract: 0.1,
            c_repulse: 2.0,
            epsilon: 0.06,
            damping: 1.0,
            dt: 0.06,
            max_step: 50.0,
            is_running: true,
        }
    }
}

pub struct ControlSettings {
    focus_selected: bool,
    key_input: bool,
    movement: Vec2,
}

impl Default for ControlSettings {
    fn default() -> Self {
        ControlSettings {
            focus_selected: true,
            key_input: true,
            movement: Vec2::ZERO,
        }
    }
}

pub struct NodeEditor {
    page_title: String,
}

impl Default for NodeEditor {
    fn default() -> Self {
        NodeEditor {
            page_title: String::new(),
        }
    }
}

pub struct StyleSettings {
    labels: bool,
}

impl Default for StyleSettings {
    fn default() -> Self {
        StyleSettings { labels: true }
    }
}

pub struct InternetStatus(InternetStatusInner);

impl InternetStatus {
    fn get_base(&self, client: &WikipediaClient) -> Result<(), wikipedia_graph::HttpError> {
        client
            .get(
                Url::parse("https://wikipedia.org")
                    .expect("Failed to parse url 'https://wikipedia.org"),
            )
            .map(|_| ())
    }

    fn update(&mut self, client: &WikipediaClient) -> &mut Self {
        match self.0 {
            InternetStatusInner::Available => {}
            InternetStatusInner::Unavailable {
                wait_time,
                last_retry,
                wait_max: _,
                error: _,
            } => {
                if Instant::now().duration_since(last_retry) > wait_time {
                    self.test_internet(client);
                }
            }
        }

        self
    }

    fn set_unavailable(&mut self, wait_time: Duration, wait_max: Duration, error: HttpError) {
        self.0 = InternetStatusInner::Unavailable {
            wait_time,
            last_retry: Instant::now(),
            wait_max,
            error: error,
        }
    }

    fn test_internet(&mut self, client: &WikipediaClient) {
        match self.get_base(client) {
            Ok(_) => {
                info!("Internet available");
                self.0 = InternetStatusInner::Available
            }
            Err(e) => {
                error!("Internet test failed: {e}");

                match self.0 {
                    InternetStatusInner::Available => {
                        self.set_unavailable(Duration::from_secs(5), Duration::from_mins(1), e)
                    }
                    InternetStatusInner::Unavailable {
                        wait_time: _,
                        last_retry: _,
                        wait_max: _,
                        error: _,
                    } => self.0.reset_unavailable(e),
                }
            }
        }
    }

    fn try_set_unavailable(&mut self, wait_time: Duration, wait_max: Duration, error: HttpError) {
        match self.0 {
            InternetStatusInner::Available => {
                self.0 = InternetStatusInner::Unavailable {
                    wait_time,
                    last_retry: Instant::now(),
                    wait_max,
                    error,
                }
            }
            _ => {}
        }
    }
}

enum InternetStatusInner {
    Available,
    Unavailable {
        wait_time: Duration,
        last_retry: Instant,
        wait_max: Duration,
        error: HttpError,
    },
}

impl InternetStatusInner {
    fn reset_unavailable(&mut self, new_error: HttpError) {
        match self {
            InternetStatusInner::Unavailable {
                wait_time,
                last_retry,
                wait_max,
                error,
            } => {
                *error = new_error;
                *last_retry = Instant::now();
                if *wait_time < wait_max.div_f32(1.5) {
                    *wait_time = wait_time.mul_f32(1.5)
                } else {
                    *wait_time = *wait_max
                }
            }
            _ => {}
        }
    }
}

impl Default for InternetStatus {
    fn default() -> Self {
        InternetStatus(InternetStatusInner::Available)
    }
}

const USER_AGENT: &str = "wikipedia-egui-graph/0.1.1";

impl WikipediaGraphApp {
    pub fn with_language(self, language: Language) -> Self {
        Self { language, ..self }
    }

    pub fn new(_: &CreationContext<'_>) -> Self {
        WikipediaGraphAppBuilder::default().build()
    }
}

impl WikipediaGraphApp {
    fn expand_node(&mut self, index: NodeIndex) {
        Self::expand_node_with_graph(&mut self.graph, &self.client, &mut self.rng, index)
    }

    pub fn expand_node_with_graph(
        graph: &mut Graph<WikipediaPage>,
        client: &WikipediaClient,
        rng: &mut Rng,
        index: NodeIndex,
    ) {
        match graph.try_expand_node(index, client) {
            Err(e) => error!("Request failed: {e}"),
            Ok(Some(indicies)) => {
                let parent_pos = graph
                    .node(index)
                    .map(|node| node.location())
                    .unwrap_or(Pos2::ZERO);

                for index in indicies {
                    let node = graph.node_mut(index).expect("Failed to find added nodes");

                    let pos = Pos2::new(rng.f32().clamp(-1.0, 1.0), rng.f32().clamp(-1.0, 1.0));

                    node.set_location(pos + parent_pos.to_vec2());

                    let title = node.payload().title();

                    node.set_label(title);
                }
            }
            Ok(None) => error!("Expanded node not found"),
        }
    }

    fn focus_selected(&self, ui: &mut Ui) {
        if let Some(selected_node) = self.selected_node {
            let mut meta = Metadata::load(ui);

            self.focused_node_from_meta(ui, &mut meta, selected_node);

            meta.save(ui);
        }
    }

    fn focused_node_from_meta(&self, ui: &Ui, meta: &mut Metadata, index: NodeIndex) {
        self.focus_point_from_meta(
            ui,
            meta,
            self.graph.node(index).unwrap().location().to_vec2(),
        );
    }

    fn focus_point_from_meta(&self, ui: &Ui, meta: &mut Metadata, point: Vec2) {
        let pos = ui.max_rect().center().to_vec2() - point * meta.zoom;

        meta.pan = pos;
    }

    fn select_random(&mut self) {
        let count = self.graph.node_count();

        let node_index = self.rng.usize(0..count);

        self.selected_node = Some(NodeIndex::new(node_index))
    }

    fn expand_random(&mut self) {
        self.select_random();

        let selected = self.selected_node.unwrap();

        self.expand_node(selected);
    }

    fn remove_selected(&mut self) {
        if let Some(selected) = self.selected_node {
            self.remove_node(selected);
        }
    }

    fn remove_node(&mut self, index: NodeIndex) {
        self.selected_node = None;

        self.graph.remove_node(index);
    }

    fn update_position_from_meta(&mut self, meta: &mut Metadata) {
        meta.pan += self.control_settings.movement
    }

    fn update_position(&mut self, ui: &mut Ui) {
        let mut meta = Metadata::load(ui);

        self.update_position_from_meta(&mut meta);

        meta.save(ui);
    }

    fn url_of(&self, index: NodeIndex) -> Option<Url> {
        Some(self.url_of_page(self.graph.node(index)?.payload()))
    }

    fn url_of_page(&self, page: &WikipediaPage) -> Url {
        page.url_with_lang(self.language)
            .expect("Selected language has no iso 639-1 encoding")
    }

    pub fn expand_connected_nodes(&mut self, index: NodeIndex) {
        for index in Self::connected_nodes(&self.graph, index, petgraph::Direction::Outgoing)
            .collect::<Vec<_>>()
        {
            self.expand_node(index);
        }
    }
}

impl App for WikipediaGraphApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        match &self.internet_status.update(&self.client).0 {
            InternetStatusInner::Available => {
                self.frame_counter.update_fps();

                egui::CentralPanel::default().show(ctx, |ui| {
                    if self.control_settings.key_input {
                        self.keybinds(ui);

                        self.update_position(ui);
                    }

                    if self.initialization > 0 {
                        let mut meta = Metadata::load(ui);

                        meta.zoom = 2.0;

                        meta.save(ui);

                        self.initialization -= 1;
                    }

                    let style = SettingsStyle::new().with_labels_always(self.style_settings.labels);

                    #[cfg(not(target_arch = "wasm32"))]
                    let event = self.event_reader.try_recv().ok();

                    #[cfg(target_arch = "wasm32")]
                    let event = self.event_buffer.borrow_mut().pop();

                    if let Some(event) = event {
                        match event {
                            Event::NodeClick(event) => {
                                self.selected_node = Some(NodeIndex::new(event.id))
                            }
                            Event::NodeDoubleClick(event) => {
                                let parent_index = NodeIndex::new(event.id);

                                self.expand_node(parent_index);
                            }
                            // Event::Pan(pan) => self.pan = pan.new_pan,
                            // Event::Zoom(zoom) => self.zoom = zoom.new_zoom,
                            _ => {}
                        }
                    }

                    if self.control_settings.focus_selected {
                        self.focus_selected(ui);
                    }

                    let mut state = egui_graphs::GraphView::<
                        (),
                        (),
                        petgraph::Directed,
                        petgraph::stable_graph::DefaultIx,
                        egui_graphs::DefaultNodeShape,
                        egui_graphs::DefaultEdgeShape,
                        FruchtermanReingoldWithCenterGravityState,
                        LayoutForceDirected<FruchtermanReingoldWithCenterGravity>,
                    >::get_layout_state(ui);

                    let layout_settings = &self.layout_settings;
                    state.base.c_repulse = layout_settings.c_repulse;
                    state.base.k_scale = layout_settings.k_scale;
                    state.base.c_attract = layout_settings.c_attract;
                    state.base.damping = layout_settings.damping;
                    state.base.epsilon = layout_settings.epsilon;
                    state.base.dt = layout_settings.dt;
                    state.base.max_step = layout_settings.max_step;
                    state.base.is_running = layout_settings.is_running;

                    let mut view = GraphView::<
                        WikipediaPage,
                        _,
                        _,
                        _,
                        _,
                        _,
                        FruchtermanReingoldWithCenterGravityState,
                        LayoutForceDirected<FruchtermanReingoldWithCenterGravity>,
                    >::new(&mut self.graph)
                    .with_interactions(&self.interaction_settings)
                    .with_navigations(&self.navigation_settings)
                    .with_styles(&style);

                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        view = view.with_event_sink(&self.event_writer);
                    }

                    #[cfg(target_arch = "wasm32")]
                    {
                        view = view.with_event_sink(&self.event_buffer);
                    }

                    egui_graphs::GraphView::<
                        (),
                        (),
                        petgraph::Directed,
                        petgraph::stable_graph::DefaultIx,
                        egui_graphs::DefaultNodeShape,
                        egui_graphs::DefaultEdgeShape,
                        FruchtermanReingoldWithCenterGravityState,
                        LayoutForceDirected<FruchtermanReingoldWithCenterGravity>,
                    >::set_layout_state(ui, state);

                    ui.add(&mut view);
                });

                egui::SidePanel::right("right")
                    .default_width(300.0)
                    .min_width(300.0)
                    .show(ctx, |ui| {
                        self.perf(ui);
                        ui.collapsing("Layout Settings", |ui| self.layout_settings(ui));
                        ui.collapsing("Control Settings", |ui| self.control_settings(ui));
                        ui.collapsing("Random Controls", |ui| self.random_controls(ui));
                        ui.collapsing("Node Controls", |ui| self.node_editor(ui));
                        ui.collapsing("Style Settings", |ui| self.style_settings(ui));
                    });

                if let Some(node_index) = self.selected_node {
                    egui::SidePanel::left("left")
                        .default_width(200.0)
                        .min_width(200.0)
                        .show(ctx, |ui| {
                            self.node_details_ui(ui, node_index);
                        });
                }
            }
            InternetStatusInner::Unavailable {
                wait_time: retry_time,
                last_retry,
                wait_max: _,
                error,
            } => {
                if egui::CentralPanel::default()
                    .show(ctx, |ui| {
                        Self::internet_unavailable_ui(
                            ui,
                            (retry_time.clone()
                                - Instant::now().duration_since(last_retry.clone()))
                            .as_secs_f32(),
                            error.to_string(),
                        )
                    })
                    .inner
                {
                    self.internet_status.test_internet(&self.client);
                }
            }
        }
    }
}
