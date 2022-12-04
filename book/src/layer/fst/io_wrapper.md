# IO Adapter for general file descriptor - IoWrapper

Other than `Future` object handling, we also need IO source to make
`async` a thing as IO is the reason why we need asynchronous runtime.

In this section, we will talk about:
1. General IO handling
2. `IoWrapper` design
