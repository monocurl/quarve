#import <Cocoa/Cocoa.h>

uint8_t const*
back_app_storage_directory(uint8_t const* app_name) {
    static __thread NSString *threadLocalStorageDirectory = nil;

    if (!threadLocalStorageDirectory) {
        NSArray *paths = NSSearchPathForDirectoriesInDomains(NSApplicationSupportDirectory, NSUserDomainMask, YES);
        NSString *basePath = ([paths count] > 0) ? paths[0] : nil;

        if (basePath) {
            NSString *appName = [NSString stringWithUTF8String:(char const*) app_name];
            NSString *appPath = [basePath stringByAppendingPathComponent:appName];

            NSFileManager *fileManager = [NSFileManager defaultManager];
            if (![fileManager fileExistsAtPath:appPath]) {
                NSError *error = nil;
                [fileManager createDirectoryAtPath:appPath withIntermediateDirectories:YES attributes:nil error:&error];
                if (error) {
                    NSLog(@"Error creating app-specific directory: %@", error);
                    return nil;
                }
            }

            threadLocalStorageDirectory = [appPath copy];
        }
    }

    return (uint8_t const*) [threadLocalStorageDirectory UTF8String];
}