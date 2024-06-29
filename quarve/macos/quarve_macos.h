#pragma once

typedef struct fat_pointer {
    void const *p0;
    void const *p1;
} fat_pointer;

typedef struct size {
    double w, h;
} size;