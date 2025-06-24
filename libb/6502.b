exit(code) {
    0xFFFC(code);
}

abort() {
    0xFFFC(69);
}

putchar(c) {
    0xFFEF(c);
}

malloc() {
    return(0x0200);
}

// TODO: Try to implement this function with assembly
// Problem with this implementation is that it is not
// mapped to the operator
div (a, b) {
    auto d;
    d = 0; while(a >= b) {
        a = a - b;
        d++;
    }
    return (d);
}

// TODO: Try to implement this function with assembly
// Problem with this implementation is that it is not
// mapped to the operator
rem (a, b) {
    auto d;
    while(a >= b) {
        a = a - b;
    }
    return (a);
}

printn(n, b) {
    auto a, c, d;

    // Simple implementation of the reminder (because too
    // difficult to implement directly in assembly)

    if(a=div(n, b)) /* assignment, not test for equality */
        printn(a, b); /* recursive */
    c = rem(n,b) + '0';
    if (c > '9') c += 7;
    putchar(c);
}

// TODO: add other arguments
printf(str, x1, x2, x3, x4, x5, x6, x7, x8, x9) {
    extrn char;
    auto i, j, arg, c;
    i = 0;
    j = 0;

    arg = &x1;

    c = char(str, i);
    while (c) {
        if (c == '\n') {
            putchar(0xD); // \r
        }
        if(c == '%') {
            i += 1;
            c = char(str, i);
            if (c == 0) {
                return;
            } else if (c == 'd') {
                printn(*arg, 10);
            } else if (c == 'p') {
                printn(*arg, 16);
            } else if (c == 'c') {
                putchar(*arg);
            } else if (c == 's') { /* clobbers `c`, the last one */
                while (c = char(*arg, j++)) {
                    putchar(c);
                }
            } else {
                putchar('%');
                arg += 2; /* word size */
            }
            arg -= 2; /* word size */
        } else {
            putchar(c); // ECHO
        }
        i++;
        c = char(str, i);
    }
}

strlen(str) {
    extrn char;
    auto i, c;
    i = 0;

    c = char(str, i);
    while (c) {
        i++;
        c = char(str, i);
    }
    return (i);
}

toupper(c) {
    if (c < 'a') {
        return (c);
    }
    if (c > 'z') {
        return (c);
    }
    return (c - 'a' + 'A');
}

// TODO: see how to implement it with assembly
// to understand if there is better memory usage.
// This will probably generate more instructions
// then needed.
lchar(str, i, c) {
    auto ptr;
    ptr = str + i;
    *ptr = *ptr&0xFF00;
    *ptr += c;
}
