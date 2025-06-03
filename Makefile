BUILD=build
SRC=src

CRUST_FLAGS=-g --edition 2021 -C opt-level=0 -C panic="abort"

RSS=\
	$(SRC)/arena.rs \
	$(SRC)/b.rs \
	$(SRC)/crust.rs \
	$(SRC)/flag.rs \
	$(SRC)/lexer.rs \
	$(SRC)/nob.rs \
	$(SRC)/codegen/fasm_x86_64.rs \
	$(SRC)/codegen/gas_aarch64_linux.rs \
	$(SRC)/codegen/uxn.rs \
	$(SRC)/codegen/ir.rs \
	$(SRC)/codegen/mod.rs

LINUX_TESTS=\
	$(BUILD)/tests/args6 \
	$(BUILD)/tests/compare \
	$(BUILD)/tests/deref_assign \
	$(BUILD)/tests/divmod \
	$(BUILD)/tests/e \
	$(BUILD)/tests/forward-declare \
	$(BUILD)/tests/goto \
	$(BUILD)/tests/hello \
	$(BUILD)/tests/inc_dec \
	$(BUILD)/tests/lexer \
	$(BUILD)/tests/literals \
	$(BUILD)/tests/minus_2 \
	$(BUILD)/tests/recursion \
	$(BUILD)/tests/ref \
	$(BUILD)/tests/return \
	$(BUILD)/tests/ternary-side-effect \
	$(BUILD)/tests/ternary \
	$(BUILD)/tests/unary_priority \
	$(BUILD)/tests/vector

MINGW32_TESTS=\
	$(BUILD)/tests/args6.exe \
	$(BUILD)/tests/compare.exe \
	$(BUILD)/tests/deref_assign.exe \
	$(BUILD)/tests/divmod.exe \
	$(BUILD)/tests/e.exe \
	$(BUILD)/tests/forward-declare.exe \
	$(BUILD)/tests/goto.exe \
	$(BUILD)/tests/hello.exe \
	$(BUILD)/tests/inc_dec.exe \
	$(BUILD)/tests/lexer.exe \
	$(BUILD)/tests/literals.exe \
	$(BUILD)/tests/minus_2.exe \
	$(BUILD)/tests/recursion.exe \
	$(BUILD)/tests/ref.exe \
	$(BUILD)/tests/return.exe \
	$(BUILD)/tests/ternary-side-effect.exe \
	$(BUILD)/tests/ternary.exe \
	$(BUILD)/tests/unary_priority.exe \
	$(BUILD)/tests/vector.exe

LINUX_OBJS=\
	$(BUILD)/nob.linux.o \
	$(BUILD)/flag.linux.o \
	$(BUILD)/libc.linux.o \
	$(BUILD)/arena.linux.o

MINGW32_OBJS=\
	$(BUILD)/nob.mingw32.o \
	$(BUILD)/flag.mingw32.o \
	$(BUILD)/libc.mingw32.o \
	$(BUILD)/arena.mingw32.o

$(BUILD)/b: $(RSS) $(LINUX_OBJS) | $(BUILD)
	rustc $(CRUST_FLAGS) -C link-args="$(LINUX_OBJS) -lc -lgcc" $(SRC)/b.rs -o $(BUILD)/b

$(BUILD)/%.linux.o: ./thirdparty/%.c | $(BUILD)
	clang -fPIC -g -c $< -o $@

# Cross-compilation on Linux to Windows using mingw32-w64
# Invoked on demand by `make ./build/b.exe`
$(BUILD)/b.exe: $(RSS) $(MINGW32_OBJS) | $(BUILD)
	rustc $(CRUST_FLAGS) --target x86_64-pc-windows-gnu -C link-args="$(MINGW32_OBJS) -lmingwex -lmsvcrt -lkernel32" $(SRC)/b.rs -o $(BUILD)/b

$(BUILD)/%.mingw32.o: ./thirdparty/%.c | $(BUILD)
	x86_64-w64-mingw32-gcc -fPIC -g -c $< -o $@

$(BUILD):
	mkdir -pv $(BUILD)

.PHONY: test
test: $(LINUX_TESTS)

$(BUILD)/tests/%: ./tests/%.b ./std/test.b $(BUILD)/b FORCE | $(BUILD)/tests
	$(BUILD)/b -run -o $@ $< ./std/test.b

test-mingw32: $(MINGW32_TESTS)

$(BUILD)/tests/%.exe: ./tests/%.b ./std/test.b $(BUILD)/b FORCE | $(BUILD)/tests
	$(BUILD)/b -t fasm-x86_64-windows -run -o $@ $< ./std/test.b

$(BUILD)/tests:
	mkdir -pv $(BUILD)/tests

# https://www.gnu.org/software/make/manual/html_node/Force-Targets.html
FORCE:
