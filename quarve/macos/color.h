#pragma once
#import <Cocoa/Cocoa.h>

typedef struct color {
    uint8_t r, g, b, a;
} color;

static inline CGColorRef
color_to_cg_color(color c)
{
    CGFloat comps[] = {c.r / 255.0, c.g / 255.0, c.b / 255.0, c.a / 255.0};
    return CGColorCreate(CGColorSpaceCreateDeviceRGB(), comps);
}
