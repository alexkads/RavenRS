# ravenrs-lottie

A Lottie player of our own for RavenRS. It reads the JSON of a Lottie file and works out, for any moment, the curves to draw, as `PathFill`s for the display list. `ravenrs-render` draws them on the GPU with the exact coverage of each pixel, so they stay sharp at any size. The `Lottie` widget of `ravenrs-widgets` is how an app shows one.

It has no GPU and no widgets, so it runs in tests, on servers and in WebAssembly.

It plays shapes, masks, mattes, precompositions, time remapping, trims, dashes, repeaters, gradients, images inside the file and the Fill effect. `Animation::unsupported` says what a file asks for that the player does not do yet, such as expressions, text layers, 3D, blend modes and other effects, so that nothing is left wrong without notice. A `.lottie` archive is a zip: unpack the JSON from it first.

`crates/ravenrs/tests/suite/lottie_corpus.rs` compares real files, picture by picture, with lottie-web.
