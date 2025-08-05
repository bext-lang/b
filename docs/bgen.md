# B Codegens

The architecture of the B compiler attempts to loosely decouple the front end (the part that generates Intermediate Representation (IR) from the Source Code) and the back end (the part that generates the final program from the IR) so to make it is easy to add more custom target platforms.

When it comes to the back end there are two main terms to consider:
1. **Target** is whatever you select using the `-t` flag of both `b` and `btest` programs. To get the list of all available targets use flag `-tlist` on any of them.
2. **Codegen** is a collection of Targets implemented as a submodule of the `codegen` module. All codegens are located in the `<root>/src/codegen/` folder.

The reason it is organized like that is because some targets share a lot of code with each other. Especially when they are implemented for the same CPU architecture. For example targets `gas-x86_64-linux`, `gas-x86_64-windows`, and `gas-x86_64-darwin` are part of a single codegen `gas_x86_64` and differ only in how they pass arguments to function calls according their platform ABIs.

Any valid Rust module under `<root>/src/codegen/` folder is automatically picked up by the build system and plugged as an additional codegen. For an example of a "pluggable" codegen see [https://github.com/bext-lang/dotnet_mono](https://github.com/bext-lang/dotnet_mono). The way you plug it in is

```console
$ git clone https://github.com/bext-lang/b
$ cd b/src/codegen/
$ git clone https://github.com/bext-lang/dotnet-mono
$ cd ../../
$ make
$ ./build/b -t ilasm-mono ./examples/hello_world.b -run
```

For an example of a minimal pluggable codegen see `<root>/src/codegen/noop.rs`.
