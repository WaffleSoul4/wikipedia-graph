# Graphing [Wikipedia](https://wikipedia.org)!

Made by Waffleleroo <3

[![Latest version](https://img.shields.io/crates/v/wikipedia-graph.svg)](https://crates.io/crates/wikipedia-graph)
[![Documentation](https://docs.rs/wikipedia-graph/badge.svg)](https://docs.rs/wikipedia-graph)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Unlicense-blue.svg)

---

## Why?

There are a ton of other wikipedia graphing tools out there! Why would you ever think of using this one over the others? 
Well, this graph is interactive, multilingual, multi-platform (native and web), highly customisable, and also a well documented crate at its core.

## How to use it

### Native

The simplest way to use it is like any other app on your computer. It does require the installation of the rust toolchain, but that's quite simple. 
Follow the instructions here to install the rust toolchain: https://rust-lang.org/tools/install

After that, try cloning the repository with `git clone https://github.com/WaffleSoul4/wikipedia-graph.git`.

Next, run `cd wikipedia-graph` followed by `cargo run --release --bin wikipedia-graph-native` to run the app.

### Web

You can also host a web server which hosts a [webassembly](https://en.wikipedia.org/wiki/WebAssembly) version of the project.

After cloning the repository, run `cd wikipedia-graph/crates/wikipedia-graph-web`. To build and run the server, run `./build_web.sh --release` & `./run_web.sh`.

## Here look at this code example!

```rust 
let mut page = WikipediaPage::from_title("Waffle");
let client = WikipediaClient::default();

let (response_sender, response_reciever) = channel::<Result<WikipediaPage, HttpError>>();

page.load_page_text(&client, move |response| response_sender.send(response).expect("Failed to send response to main thread"));

page = response_reciever.recv()??;

println!("Page title: {}", page.title());

for page in page.try_get_linked_pages().unwrap() {
    println!("Connects to {}", page.title());
}
```

## Tools this project uses

- [egui_graphs](https://github.com/blitzarx1/egui_graphs): used for showing the graph view
- [egui](https://github.com/emilk/egui): Genuinely one of the greatest ui tools ever with multiplatform support
- [ehttp](https://github.com/emilk/ehttp): A minimal, callback-based http framework
- [static-web-server](https://github.com/static-web-server/static-web-server): Used by default to host on a web server

## Documentation

Try [docs.rs](https://docs.rs/crate/wikipedia-graph/latest), if that doesn't work, you can clone the repository and generate the docs with `cargo doc`

## Common Issues

### I can't pan around the graph
Go under control settings and uncheck 'focus selected node'.

### The nodes are vibrating a little too much
Just click layout settings and lower the delta time (dt). The speed of the simulation will also decrease, but I don't really know another way.

### Where did all the frames go?
Sorry. If you know any ways to speed it up, please tell me. A few ways to *marginally* increase the frame rate are to disable 'show labels' under style settings.

---

Licensed under the [MIT License](LICENSE-MIT) or [Unlicense](UNLICENSE)

This project is completely unofficial.
