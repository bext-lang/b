BUILD=build
THIRDPARTY=thirdparty
SRC=src
EXAMPLES=examples

$(BUILD)/b: $(SRC)/arena.rs $(SRC)/b.rs $(SRC)/crust.rs $(SRC)/flag.rs $(SRC)/nob.rs $(SRC)/stb_c_lexer.rs $(SRC)/codegen/fasm_x86_64_linux.rs $(SRC)/codegen/gas_aarch64_linux.rs $(SRC)/codegen/html_js.rs $(SRC)/codegen/html_js_template.tt $(SRC)/codegen/ir.rs $(SRC)/codegen/mod.rs $(BUILD)/nob.o $(BUILD)/stb_c_lexer.o $(BUILD)/flag.o $(BUILD)/arena.o
	rustc --edition 2021 -g -C opt-level=z -C link-args="$(BUILD)/nob.o $(BUILD)/stb_c_lexer.o $(BUILD)/flag.o $(BUILD)/arena.o -lc -lgcc" -C panic="abort" $(SRC)/b.rs -o $(BUILD)/b

$(BUILD)/nob.o: $(THIRDPARTY)/nob.h | $(BUILD)
	clang -fPIC -g -x c -DNOB_IMPLEMENTATION -c $(THIRDPARTY)/nob.h -o $(BUILD)/nob.o

$(BUILD)/stb_c_lexer.o: $(THIRDPARTY)/stb_c_lexer.h | $(BUILD)
	clang -g -x c -DSTB_C_LEXER_IMPLEMENTATION -c $(THIRDPARTY)/stb_c_lexer.h -o $(BUILD)/stb_c_lexer.o

$(BUILD)/flag.o: $(THIRDPARTY)/flag.h | $(BUILD)
	clang -g -x c -DFLAG_IMPLEMENTATION -c $(THIRDPARTY)/flag.h -o $(BUILD)/flag.o

$(BUILD)/arena.o: $(THIRDPARTY)/arena.h | $(BUILD)
	clang -g -x c -DARENA_IMPLEMENTATION -c $(THIRDPARTY)/arena.h -o $(BUILD)/arena.o

$(BUILD):
	mkdir -pv $(BUILD)

# TODO: use nob to build the project
