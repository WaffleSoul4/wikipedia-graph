pub mod builder;
mod ui;

use crate::builder::WikipediaGraphAppBuilder;
use eframe::{App, CreationContext};
use egui::{CollapsingHeader, Context, Pos2, Ui, Vec2};
use egui_graphs::{
    FruchtermanReingoldWithCenterGravity, FruchtermanReingoldWithCenterGravityState, Graph,
    GraphView, LayoutForceDirected, MetadataFrame, SettingsInteraction, SettingsNavigation,
    SettingsStyle, events::Event,
};
use fastrand::Rng;
use log::warn;
use petgraph::graph::NodeIndex;
use std::sync::{Arc, Mutex};
use web_time::{Duration, Instant};
use wikipedia_graph::{
    HttpError, Url, WikiLanguage, WikipediaClient, WikipediaGraph, WikipediaPage,
};

type StoreType<T> = Arc<Mutex<Option<Result<T, HttpError>>>>;

fn store_callback<T>(store: StoreType<T>) -> impl Fn(Result<T, HttpError>) {
    move |response| match store.lock() {
        Ok(mut t) => *t = Some(response),
        Err(mut e) => {
            warn!("Waiting on mutex...");
            **e.get_mut() = Some(response)
        }
    }
}

fn store_callback_vec<T>(
    data: Arc<Mutex<Vec<(NodeIndex, Result<T, HttpError>, NodeAction)>>>,
    index: NodeIndex,
    action: NodeAction,
) -> impl Fn(Result<T, HttpError>) {
    move |response| match data.lock() {
        Ok(mut data) => {
            data.push((index, response, action));
        }
        Err(mut e) => {
            warn!("Waiting on mutex...");
            e.get_mut().push((index, response, action));
        }
    }
}

pub struct WikipediaGraphApp {
    pub graph: Graph<WikipediaPage>,
    pub interaction_settings: SettingsInteraction,
    pub navigation_settings: SettingsNavigation,
    pub layout_settings: LayoutSettings,
    #[cfg(not(target_arch = "wasm32"))]
    pub event_writer: crossbeam::channel::Sender<Event>,
    #[cfg(not(target_arch = "wasm32"))]
    pub event_reader: crossbeam::channel::Receiver<Event>,
    #[cfg(target_arch = "wasm32")]
    pub event_buffer: std::rc::Rc<std::cell::RefCell<Vec<Event>>>,
    pub client: WikipediaClient,
    pub frame_counter: FrameCounter,
    pub control_settings: ControlSettings,
    pub rng: Rng,
    pub node_editor: NodeEditor,
    pub style_settings: StyleSettings,
    pub initialization: u8,
    pub internet_status: InternetStatus,
    pub language: WikiLanguage,
    pub search_data: SearchData,
    pub node_stores: Arc<Mutex<Vec<(NodeIndex, Result<WikipediaPage, HttpError>, NodeAction)>>>,
    pub test_store: StoreType<()>,
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
    fn unavailable() -> Self {
        InternetStatus(InternetStatusInner::Unavailable {
            wait_time: Duration::from_secs(5),
            last_retry: Instant::now(),
            wait_max: Duration::from_mins(1),
            error: HttpError::PageNotFound,
            waiting: false,
        })
    }

    fn get_base(&self, client: &WikipediaClient, store: StoreType<()>) {
        client.get_api_base(store_callback(store));
    }

    fn update(
        &mut self,
        client: &WikipediaClient,
        store: Arc<Mutex<Option<Result<(), HttpError>>>>,
    ) -> &mut Self {
        match self.0 {
            InternetStatusInner::Available => {}
            InternetStatusInner::Unavailable {
                wait_time,
                last_retry,
                wait_max: _,
                error: _,
                waiting,
            } => {
                if Instant::now().duration_since(last_retry) > wait_time {
                    self.test_internet(client, store);
                } else if waiting {
                    if let Ok(mut response) = store.try_lock() {
                        if let Some(response) = response.take() {
                            match response {
                                Ok(()) => self.0 = InternetStatusInner::Available,
                                Err(e) => self.0.reset_unavailable(e),
                            }
                        }
                    }
                }
            }
        }

        self
    }

    #[allow(unused)] // Haven't set up proper network handling
    fn set_unavailable(&mut self, wait_time: Duration, wait_max: Duration, error: HttpError) {
        self.0 = InternetStatusInner::Unavailable {
            wait_time,
            last_retry: Instant::now(),
            wait_max,
            error: error,
            waiting: false,
        }
    }

    fn test_internet(&mut self, client: &WikipediaClient, store: StoreType<()>) {
        self.get_base(client, store);

        self.0.set_waiting();
    }

    #[allow(unused)]
    fn try_set_unavailable(&mut self, wait_time: Duration, wait_max: Duration, error: HttpError) {
        match self.0 {
            InternetStatusInner::Available => {
                self.0 = InternetStatusInner::Unavailable {
                    wait_time,
                    last_retry: Instant::now(),
                    wait_max,
                    error,
                    waiting: false,
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
        waiting: bool,
    },
}

impl InternetStatusInner {
    fn set_waiting(&mut self) {
        if let Self::Unavailable { waiting, .. } = self {
            *waiting = true;
        }
    }

    fn reset_unavailable(&mut self, new_error: HttpError) {
        match self {
            InternetStatusInner::Unavailable {
                wait_time,
                last_retry,
                wait_max,
                error,
                waiting: _,
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

pub struct SearchData {
    page_count: usize,
    query: String,
    last_update: Instant,
    stored_pages: Vec<(String, NodeIndex)>,
}

impl SearchData {
    fn time_since_update(&self) -> Duration {
        Instant::now().duration_since(self.last_update)
    }

    fn search_pages<'a>(
        &self,
        pages: Vec<(&'a WikipediaPage, NodeIndex<u32>)>,
    ) -> Vec<(String, NodeIndex<u32>)> {
        let fuse = fuse_rust::Fuse::default();

        let pages = pages
            .iter()
            .map(|(page, index)| (page.title(), index))
            .collect::<Vec<_>>();

        let mut filtered =
            fuse.search_text_in_iterable(&self.query, pages.iter().map(|(page, _)| page.as_str()));

        filtered.sort_by(|result, result2| {
            result
                .score
                .partial_cmp(&result2.score)
                .expect("A page had an incomparable score to the search result")
        });

        filtered
            .into_iter()
            .take(self.page_count)
            .map(|result| result.index)
            .filter_map(|index| pages.get(index))
            .map(|(page, index)| (page.clone(), **index))
            .collect()
    }

    fn get_searched_pages(
        &mut self,
        indicies: Vec<(&WikipediaPage, NodeIndex<u32>)>,
    ) -> Vec<(String, NodeIndex<u32>)> {
        // This is annoying to do
        if self.time_since_update() > Duration::from_millis(200) {
            let pages = self.search_pages(indicies);

            self.stored_pages = pages.clone();

            pages
        } else {
            self.stored_pages.clone()
        }
    }
}

impl Default for SearchData {
    fn default() -> Self {
        SearchData {
            query: String::new(),
            last_update: Instant::now(),
            stored_pages: Vec::with_capacity(10),
            page_count: 10,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum NodeAction {
    Expand,
    None,
}

const USER_AGENT: &str = "wikipedia-egui-graph/0.1.1";

impl WikipediaGraphApp {
    pub fn with_language(self, language: WikiLanguage) -> Self {
        Self { language, ..self }
    }

    pub fn new(_: &CreationContext<'_>) -> Self {
        WikipediaGraphAppBuilder::default().build()
    }
}

impl WikipediaGraphApp {
    pub fn update_nodes_from_store(
        store: &mut Arc<Mutex<Vec<(NodeIndex, Result<WikipediaPage, HttpError>, NodeAction)>>>,
        graph: &mut Graph<WikipediaPage>,
        rng: &mut Rng,
    ) {
        match store.try_lock() {
            Ok(mut store) => {
                let len = store.len();

                store
                    .drain(0..len)
                    .into_iter()
                    .filter_map(|(index, response, action)| match response {
                        Ok(t) => Some((index, t, action)),
                        Err(e) => {
                            warn!("Request failed: {e}");
                            None
                        }
                    })
                    .for_each(|(index, page, action)| match graph.node_mut(index) {
                        Some(node) => {
                            node.set_label(page.title());
                            *node.payload_mut() = page;

                            match action {
                                NodeAction::Expand => {
                                    Self::expand_node_with_graph(graph, rng, index);
                                }
                                NodeAction::None => {}
                            }
                        }
                        None => warn!(
                            "Unable to find the node for page '{}' at index '{}'",
                            page.title(),
                            index.index()
                        ),
                    });
            }
            Err(e) => warn!("Main thread failed to get lock: {e}"),
        }
    }

    fn expand_node(&mut self, index: NodeIndex) {
        self.load_node(index, NodeAction::Expand);
    }

    pub fn expand_node_with_graph(
        graph: &mut Graph<WikipediaPage>,
        rng: &mut Rng,
        index: NodeIndex,
    ) {
        match graph.try_expand_node(index) {
            Some(indicies) => {
                let parent_pos = graph
                    .node(index)
                    .map(|node| node.location())
                    .unwrap_or(Pos2::ZERO);

                for index in indicies {
                    let node = graph
                        .node_mut(index)
                        .expect("Failed to find newly added nodes");

                    let pos = Pos2::new(rng.i8(-5..5) as f32, rng.i8(-5..5) as f32);

                    node.set_location(pos + parent_pos.to_vec2());

                    node.set_label(node.payload().title());
                }
            }
            None => warn!("Failed to expand node: node not found at index"),
        }
    }

    fn focus_selected(&self, ui: &mut Ui) {
        if let Some(selected_node) = self.selected_node() {
            let mut meta = MetadataFrame::new(None).load(ui);

            self.focused_node_from_meta(ui, &mut meta, selected_node.clone());

            meta.save(ui);
        }
    }

    fn focused_node_from_meta(&self, ui: &Ui, meta: &mut MetadataFrame, index: NodeIndex) {
        self.focus_point_from_meta(
            ui,
            meta,
            self.graph.node(index).unwrap().location().to_vec2(),
        );
    }

    fn focus_point_from_meta(&self, ui: &Ui, meta: &mut MetadataFrame, point: Vec2) {
        let pos = ui.max_rect().center().to_vec2() - point * meta.zoom;

        meta.pan = pos;
    }

    fn selected_node(&self) -> Option<&NodeIndex> {
        self.graph.selected_nodes().get(0)
    }

    fn set_selected_node(&mut self, index: Option<NodeIndex>) {
        // Deselect the previously selected node
        if let Some(index) = self.selected_node() {
            match self.graph.node_mut(index.clone()) {
                Some(node) => node.set_selected(false),
                None => warn!("Previously selected node does not exist"),
            }
        }

        if let Some(index) = index {
            match self.graph.node_mut(index) {
                Some(node) => node.set_selected(true),
                None => warn!("Failed to set the selected node: node doesn't exist"),
            }
        }
    }

    fn select_random(&mut self) {
        match self
            .rng
            .choice(self.graph.node_indicies().iter().map(|(_, index)| index))
        {
            Some(index) => self.set_selected_node(Some(index.clone())),
            None => warn!("Failed to select a random node"),
        }
    }

    fn expand_random(&mut self) {
        self.select_random();
        match self.selected_node() {
            Some(index) => self.expand_node(index.clone()),
            None => warn!("Failed to expand random node: no node was preselected"),
        }
    }

    fn remove_selected(&mut self) {
        if let Some(selected) = self.selected_node() {
            self.remove_node(selected.clone());
            self.set_selected_node(None);
        }
    }

    fn remove_node(&mut self, index: NodeIndex) {
        self.graph.remove_node(index);
    }

    fn update_position_from_meta(&mut self, meta: &mut MetadataFrame) {
        meta.pan += self.control_settings.movement
    }

    fn update_position(&mut self, ui: &mut Ui) {
        let mut meta = MetadataFrame::new(None).load(ui);

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

    pub fn load_node(&mut self, index: NodeIndex, action: NodeAction) {
        if let Some(node) = self.graph.node(index) {
            if let Err(e) = node.payload().load_page_text(
                &self.client,
                store_callback_vec(self.node_stores.clone(), index, action),
            ) {
                warn!("{e}") // Self explanatory error
            }
        }
    }
}

impl App for WikipediaGraphApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        dbg!(self.graph.selected_nodes());

        match &self
            .internet_status
            .update(&self.client, self.test_store.clone())
            .0
        {
            InternetStatusInner::Available => {
                Self::update_nodes_from_store(
                    &mut self.node_stores,
                    &mut self.graph,
                    &mut self.rng,
                );

                self.search_bar(ctx);

                self.frame_counter.update_fps();

                egui::CentralPanel::default().show(ctx, |ui| {
                    if self.control_settings.key_input {
                        self.keybinds(ui);

                        self.update_position(ui);
                    }

                    if self.initialization > 0 {
                        let mut meta = MetadataFrame::new(None).load(ui);

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
                                self.set_selected_node(Some(NodeIndex::new(event.id)))
                            }
                            Event::NodeDoubleClick(event) => {
                                let parent_index = NodeIndex::new(event.id);

                                self.load_node(parent_index, NodeAction::Expand);
                            }
                            _ => {}
                        }
                    }

                    if self.control_settings.focus_selected {
                        self.focus_selected(ui);
                    }

                    let mut state = egui_graphs::get_layout_state::<
                        FruchtermanReingoldWithCenterGravityState,
                    >(ui, None);

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

                    egui_graphs::set_layout_state(ui, state, None);

                    ui.add(&mut view);
                });

                egui::SidePanel::right("right")
                    .default_width(300.0)
                    .min_width(300.0)
                    .show(ctx, |ui| {
                        self.perf(ui);
                        ui.separator();
                        CollapsingHeader::new("Layout Settings")
                            .default_open(true)
                            .show(ui, |ui| self.layout_settings(ui));
                        CollapsingHeader::new("Controls")
                            .default_open(true)
                            .show(ui, |ui| self.control_settings(ui));
                        CollapsingHeader::new("Node Settings")
                            .default_open(true)
                            .show(ui, |ui| {
                                self.node_editor(ui);
                                self.random_controls(ui);
                            });
                        CollapsingHeader::new("Style")
                            .default_open(true)
                            .show(ui, |ui| self.style_settings(ui));
                    });

                if let Some(node_index) = self.selected_node() {
                    let selected = node_index.clone();

                    egui::SidePanel::left("left")
                        .default_width(200.0)
                        .min_width(200.0)
                        .show(ctx, |ui| {
                            self.node_details_ui(ui, selected);
                        });
                }
            }
            InternetStatusInner::Unavailable {
                wait_time: retry_time,
                last_retry,
                error,
                ..
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
                    self.internet_status
                        .test_internet(&self.client, self.test_store.clone());
                }
            }
        }
    }
}
