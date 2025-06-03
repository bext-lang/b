// -*- mode: simpc -*-

// To compile this example you need to download raylib from https://github.com/raysan5/raylib/releases
// Than pass appropriate linker flags to the b compiler.
// # Linux
//
// $ b raylib.b -L -L/path/to/raylib-version_linux_amd64/lib/ -L -l:libraylib.a -L -lm -run
//
// # Windows mingw32-w64
// > b -t fasm-x86_64-windows raylib.b -L -L$HOME/opt/raylib-version_win64_mingw-w64/lib/ -L -l:libraylib.a -L -lwinmm -L -lgdi32 -run

// TODO: Crashing during runtime when compiled with -t fasm-x86_64-windows and running via wine
main() {
    // libc
    extrn malloc;

    // Raylib
    extrn InitWindow, BeginDrawing, EndDrawing,
          WindowShouldClose, ClearBackground, DrawRectangle,
          SetTargetFPS, GetScreenWidth, GetScreenHeight,
          IsKeyPressed;

    auto word;
    word = 8;

    auto c, cs, cn;
    c              = 0;               // Current color
    cn             = 6;               // Amount of colors
    cs             = malloc(word*cn); // Color Table
    *cs            = 0xFF1818FF;      // B originally does not support hex literals actually.
    *(cs + word)   = 0xFF18FF18;
    *(cs + word*2) = 0xFFFF1818;
    *(cs + word*3) = 0xFFFFFF18;
    *(cs + word*4) = 0xFFFF18FF;
    *(cs + word*5) = 0xFF18FFFF;

    auto x, y, dx, dy, sx, sy;
    sx = sy = 100;
    x  = y  = 200;
    dx = dy = 5;

    auto paused;
    paused = 0;

    InitWindow(800, 600, "Hello, from B");
    SetTargetFPS(60);
    while (!WindowShouldClose()) {
        if (IsKeyPressed(32)) {
            paused = !paused;
        }

        if (!paused) {
            // TODO: Use logic-or || operation here instead of the bit-or
            auto nx, ny;
            nx = x + dx; if (nx < 0 | nx + sx >= GetScreenWidth())  { dx = -dx; c = (c + 1)%cn; } else x = nx;
            ny = y + dy; if (ny < 0 | ny + sy >= GetScreenHeight()) { dy = -dy; c = (c + 1)%cn; } else y = ny;
        }

        BeginDrawing();
        ClearBackground(0xFF181818);
        DrawRectangle(x, y, sx, sy, *(cs + word*c));
        EndDrawing();
    }
}
