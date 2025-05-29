# B Programming Language

> [!WARNING]
> Compiler is not fully implemented yet.

<p align=center>
  <img src="./logo/logo_strawberry.png" width=400>
</p>

<p align=right>
  <sub>Logo by Strawberry 🍓</sub>
</p>

Compiler for the B Programming Language implemented in [Crust](https://github.com/tsoding/crust)

## Dependencies

- [Rust](https://www.rust-lang.org/) - the compiler is written in it;
- [fasm](https://flatassembler.net/) - used as the compiler backend;
- [clang](https://clang.llvm.org/) - for linking with the C runtime;

## Quick Start

```console
$ make
$ ./build/b -run ./examples/01_hello_world.b
```

### Uxn

The compiler support [Uxn](https://100r.co/site/uxn.html) target. Make sure you have `uxnemu` in your `$PATH` if you want to use `-run` flag.

```console
$ ./build/b -t uxn -run ./examples/01_hello_world.b ./std/uxn.b
```

Also check out more examples at [./examples/](./examples/).

## References

- https://en.wikipedia.org/wiki/B_(programming_language)
- https://web.archive.org/web/20241214022534/https://www.bell-labs.com/usr/dmr/www/kbman.html
- https://github.com/tsoding/good_training_language
- https://www.felixcloutier.com/x86/
- https://www.scs.stanford.edu/~zyedidia/arm64/
