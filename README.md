# Graphing [Wikipedia](https://wikipedia.org)!

Made by Waffleleroo <3

---

## Common Issues (for me)

### I can't pan around the graph
Go under control settings and uncheck 'focus selected node'

### The nodes are vibrating a little too much
Just click layout settings and lower the delta time (dt). The speed of the simulation will also decrease, but I don't really know another way

### Everything is frozen
You probably tried to expand a node with a lot of links, or you pressed the 'expand connected nodes' button. If it's the prior, it shouldn't take too long to load. If it's the latter, you should find something else to do for a while. I think it takes so long because of adding every node to the graph, not the actual requests. If neither of these caused the freeze, then I don't know what caused it and you should submit an issue with details on how to reproduce it.

### The framerate is wayyyyyy to low
Sorry. If you know any ways to speed it up, please tell me. A few ways to *marginally* increase the framerate are to disable 'show labels' under style settings

---

Licensed under the [MIT License](LICENSE-MIT) or [Unlicense](UNLICENSE)

This project is completely unofficial