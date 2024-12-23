#import <Cocoa/Cocoa.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>
#import "util.h"
#import "front.h"

@interface Picker : NSObject
@property(retain) NSSavePanel* panel;
@property(retain) NSString* url;
@end

@implementation Picker
- (void)dealloc {
    if (self.url) {
        [self.url release];
    }

    [super dealloc];
}
@end

static NSMutableArray<UTType*>*
allowed_types(uint8_t const* mask)
{
    if (!mask) {
        return [[NSMutableArray alloc] init];
    }

    NSString* nstring = [NSString stringWithUTF8String:(const char*)mask];

    NSArray<NSString*>* extensions = [nstring componentsSeparatedByString:@"|"];

    NSMutableArray<UTType*>* types = [NSMutableArray arrayWithCapacity:[extensions count]];
    for (NSString* extension in extensions) {
        UTType* type = [UTType typeWithFilenameExtension:extension];
        if (type) {
            [types addObject:type];
        }
    }

    [types retain];

    return types;
}

void*
back_file_open_picker_init(uint8_t const* allowed_mask) {
    Picker* picker = [Picker alloc];
    picker.panel = [NSOpenPanel openPanel];

    NSMutableArray<UTType*>* allowed = allowed_types(allowed_mask);
    picker.panel.allowedContentTypes = allowed;
    [allowed release];

    picker.url = NULL;
    return picker;
}

uint8_t const*
back_file_open_picker_run(void *op) {
    Picker* panel = op;

    NSModalResponse response = [panel.panel runModal];
    if (response == NSModalResponseOK) {
        panel.url = panel.panel.URL.path;
        return (uint8_t const*) [panel.url UTF8String];
    }
    else {
        return NULL;
    }
}

void
back_file_open_picker_free(void *op) {
    Picker* panel = op;
    [panel release];
}

void*
back_file_save_picker_init(uint8_t const* allowed_mask) {
    Picker* picker = [Picker alloc];
    picker.panel = [NSSavePanel savePanel];

    NSMutableArray<UTType*>* allowed = allowed_types(allowed_mask);
    picker.panel.allowedContentTypes = allowed;
    [allowed release];

    picker.url = NULL;
    return picker;
}

uint8_t const*
back_file_save_picker_run(void *op) {
    return back_file_open_picker_run(op);
}

void
back_file_save_picker_free(void *op) {
    back_file_open_picker_free(op);
}
