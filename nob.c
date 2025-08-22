#define NOB_IMPLEMENTATION
#define NOB_STRIP_PREFIX
#include "./thirdparty/nob.h"

#define SRC_FOLDER   "./src/"
#define BUILD_FOLDER "./build/"

const char *thirdparty_objects[] = {
    "arena",
    "flag",
    "glob",
    "jim",
    "jimp",
    "libc",
    "nob",
    "shlex",
    "time",
};

// TODO: Support MacOS and Android.
typedef enum {
    TARGET_POSIX,
    TARGET_MINGW,
    TARGET_MSVC,
} Build_Target;

#if defined(_WIN32) && defined(_MSC_VER)
    Build_Target default_target = TARGET_MSVC;
#elif defined(_WIN32)
    Build_Target default_target = TARGET_MINGW;
#else
    Build_Target default_target = TARGET_POSIX;
#endif

typedef struct {
    Cmd cmd;
    Procs procs;
    size_t max_procs;
    String_Builder sb;
} Build_Context;

bool run_cmd(Build_Context *build) {
    Proc proc = cmd_run_async_and_reset(&build->cmd);
    if (build->max_procs) {
        return procs_append_with_flush(&build->procs, proc, build->max_procs);
    } else {
        da_append(&build->procs, proc);
        return true;
    }
}

bool build_thirdparty_objects(Build_Context *build, Build_Target target, File_Paths *object_paths) {
    for (int i = 0; i < ARRAY_LEN(thirdparty_objects); i++) {
        const char *object_name = thirdparty_objects[i];
        const char *input_path = temp_sprintf("./thirdparty/%s.c", object_name);
        switch (target) {
            case TARGET_POSIX:
            case TARGET_MINGW: {
                const char *cc = (target == TARGET_MINGW) ? "x86_64-w64-mingw32-gcc" : getenv("CC");
                const char *output_path = temp_sprintf(BUILD_FOLDER"%s.o", object_name);
                cmd_append(
                    &build->cmd,
                    cc ? cc : "cc",
                    "-fPIC",
                    "-g",
                    "-c", input_path,
                    "-o", output_path,
                );
                cmd_append(&build->cmd, "-lc", "-lgcc");
                da_append(object_paths, output_path);
            } break;

            case TARGET_MSVC: {
                const char *obj_path = temp_sprintf(BUILD_FOLDER"%s.obj", object_name);
                const char *pdb_path = temp_sprintf(BUILD_FOLDER"%s.pdb", object_name);
                cmd_append(
                    &build->cmd,
                    "cl",
                    "/nologo",
                    "/Zi",
                    "/MD",
                    "/c", input_path,
                    temp_sprintf("/Fo:%s", obj_path),
                    temp_sprintf("/Fd:%s", pdb_path),
                );
                da_append(object_paths, obj_path);
            } break;
        }
        if (!run_cmd(build)) return false;
    }
    return true;
}

const char *get_executable_ext(Build_Target target) {
    switch (target) {
        case TARGET_POSIX: return "";
        case TARGET_MINGW:
        case TARGET_MSVC:  return ".exe";
    }
}

bool build_crust_executable(Build_Context *build, Build_Target target, File_Paths object_paths, const char *program_name, const char **executable_path) {
    const char *input_path  = temp_sprintf(SRC_FOLDER"%s.rs", program_name);
    const char *output_path = temp_sprintf(BUILD_FOLDER"%s%s", program_name, get_executable_ext(target));

    if (executable_path) {
        *executable_path = output_path;
    }

    da_append_many(&build->cmd, object_paths.items, object_paths.count);
    switch (target) {
        case TARGET_POSIX: cmd_append(&build->cmd, "-lc", "-lgcc"); break;
        case TARGET_MINGW: cmd_append(&build->cmd, "-lmingwex", "-lmsvcrt", "-lkernel32"); break;
        case TARGET_MSVC:  cmd_append(&build->cmd, "msvcrt.lib", "legacy_stdio_definitions.lib"); break;
    }

    build->sb.count = 0;
    sb_append_cstr(&build->sb, "link-args=");
    cmd_render(build->cmd, &build->sb);
    sb_append_null(&build->sb);
    const char *link_args = build->sb.items;

    build->cmd.count = 0;
    cmd_append(
        &build->cmd,
        "rustc",
        "-g",
        "--edition", "2021",
        "-C", "opt-level=0",
        "-C", "panic=abort",
        "-C", link_args,
        input_path,
        "-o", output_path,
    );

    switch (target) {
        case TARGET_POSIX: break;
        case TARGET_MINGW: cmd_append(&build->cmd, "--target", "x86_64-pc-windows-gnu");  break;
        case TARGET_MSVC:  cmd_append(&build->cmd, "--target", "x86_64-pc-windows-msvc"); break;
    }

    return run_cmd(build);
}

bool target_from_cstr(const char *target_name, Build_Target *target) {
    if (strcmp(target_name, "posix") == 0) { *target = TARGET_POSIX; return true; }
    if (strcmp(target_name, "mingw") == 0) { *target = TARGET_MINGW; return true; }
    if (strcmp(target_name, "msvc") == 0)  { *target = TARGET_MSVC;  return true; }
    return false;
}

int main(int argc, char **argv) {
    NOB_GO_REBUILD_URSELF_PLUS(argc, argv, "./thirdparty/nob.h");

    Build_Context build = {0};
    Build_Target target = default_target;

    // TODO: Add usage() and help flag.
    const char *program_name = shift(argv, argc);
    while (argc) {
        const char *arg = shift(argv, argc);
        if (strcmp(arg, "-j") == 0) {
            if (!argc) {
                nob_log(ERROR, "%s: bad -j: no value provided", program_name);
                return 1;
            }
            char *max_procs = shift(argv, argc);
            char *endptr;
            build.max_procs = strtoul(max_procs, &endptr, 0);
            if (strcmp(endptr, "") != 0) {
                nob_log(ERROR, "%s: bad -j: expected an integer, got \"%s\"", program_name, max_procs);
                return 1;
            }
        } else if (strcmp(arg, "-t") == 0) {
            if (!argc) {
                nob_log(ERROR, "%s: bad -t: no value provided", program_name);
                return 1;
            }
            const char *target_name = shift(argv, argc);
            if (!target_from_cstr(target_name, &target)) {
                nob_log(ERROR, "%s: bad -t: no such target: \"%s\"", program_name, target_name);
                return 1;
            }
        } else {
            nob_log(ERROR, "%s: unexpected command line argument: \"%s\"", program_name, arg);
            return 1;
        }
    }

    if (!mkdir_if_not_exists(BUILD_FOLDER)) return 1;

    File_Paths object_paths = {0};
    if (!build_thirdparty_objects(&build, target, &object_paths)) return 1;
    if (!procs_wait_and_reset(&build.procs)) return 1;

    const char *bgen_path;
    if (!build_crust_executable(&build, target, object_paths, "bgen", &bgen_path)) return 1;
    if (!procs_wait_and_reset(&build.procs)) return 1;

    cmd_append(&build.cmd, bgen_path);
    if (!cmd_run_sync_and_reset(&build.cmd)) return 1;

    if (!build_crust_executable(&build, target, object_paths, "b", NULL)) return 1;
    if (!build_crust_executable(&build, target, object_paths, "btest", NULL)) return 1;
    if (!procs_wait_and_reset(&build.procs)) return 1;

    return 0;
}
