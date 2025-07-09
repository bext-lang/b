# B Programming Language

> [!WARNING]
> Compiler is not fully implemented yet.

<p align=center>
  <img src="./logo/logo_strawberry.png" width=400>
</p>

<p align=center>
  <sub>Logo by Strawberry 🍓</sub>
</p>

Compiler for the B Programming Language implemented in [Crust](https://github.com/tsoding/crust)

## Dependencies

- [Rust](https://www.rust-lang.org/) - the compiler is written in it;
- [fasm](https://flatassembler.net/) - used as the compiler backend;

<!-- TODO: document specific dependencies for the rest of the targets. Like mingw32-w64 and wine on Linux for Fasm_x86_64_Windows, etc. -->

## Quick Start

```console
$ make
$ ./build/b -run ./examples/hello_world.b
```

Also check out more examples at [./examples/](./examples/).
Find the project documentation at [./docs/](./docs/).

## References

- https://en.wikipedia.org/wiki/B_(programming_language)
- https://www.nokia.com/bell-labs/about/dennis-m-ritchie/kbman.html
- https://www.nokia.com/bell-labs/about/dennis-m-ritchie/bref.html
- https://www.nokia.com/bell-labs/about/dennis-m-ritchie/btut.html
- https://github.com/tsoding/good_training_language
- https://www.felixcloutier.com/x86/
- https://www.scs.stanford.edu/~zyedidia/arm64/
