use crate::{
    ControlSettings, FrameCounter, InternetStatus, LayoutSettings, NodeEditor, StyleSettings,
    USER_AGENT, WikipediaGraphApp,
};
use egui_graphs::{Graph, SettingsInteraction, SettingsNavigation};
use fastrand::Rng;
use log::{error, info};
use petgraph::prelude::StableDiGraph;
use std::time::Duration;
use wikipedia_graph::{Language, WikipediaClient, WikipediaClientConfig, WikipediaPage};

// Don't worry, I might add more
pub struct WikipediaGraphAppBuilder {
    language: Language,
}

impl Default for WikipediaGraphAppBuilder {
    fn default() -> Self {
        WikipediaGraphAppBuilder {
            language: Language::from_639_1("en").expect("en is not a valid iso"),
        }
    }
}

impl WikipediaGraphAppBuilder {
    pub fn with_language(self, language: Language) -> Self {
        Self { language, ..self }
    }

    pub fn build(self) -> WikipediaGraphApp {
        let config = WikipediaClientConfig::default()
            .language(self.language)
            .user_agent(USER_AGENT)
            .expect("User agent is invalid");

        let client =
            WikipediaClient::from_config(config).expect("Client has an invalid configuration");

        let graph = StableDiGraph::default();

        let mut graph = Graph::new(graph);

        let mut internet_status = InternetStatus::default();

        internet_status.test_internet(&client);

        let interaction_settings = SettingsInteraction::new()
            .with_node_clicking_enabled(true)
            .with_dragging_enabled(true);

        let navigation_settings = SettingsNavigation::new()
            .with_zoom_and_pan_enabled(true)
            .with_fit_to_screen_enabled(false);

        let (event_writer, event_reader) = crossbeam::channel::unbounded();

        WikipediaGraphApp {
            graph: graph,
            interaction_settings,
            navigation_settings,
            layout_settings: LayoutSettings::default(),
            event_writer,
            event_reader,
            client,
            frame_counter: FrameCounter::default(),
            selected_node: None,
            control_settings: ControlSettings::default(),
            rng: Rng::new(),
            node_editor: NodeEditor::default(),
            style_settings: StyleSettings::default(),
            initialization: 5,
            internet_status,
            language: self.language,
        }
    }
}
