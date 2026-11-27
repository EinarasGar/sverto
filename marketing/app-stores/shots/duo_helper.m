// Posts the vendor HID event Device Hub sends for the hinge slider and Rotate buttons; run inside the simulator with simctl spawn.
// Technique from github.com/artemnovichkov/hinge.
//   duo_helper angle <0-180> | orient <portrait|landscape-left|landscape-right|pud>
#import <Foundation/Foundation.h>
#include <mach/mach_time.h>
#include <unistd.h>

typedef struct __IOHIDEvent *IOHIDEventRef;
typedef struct __IOHIDEventSystemClient *IOHIDEventSystemClientRef;
extern IOHIDEventRef IOHIDEventCreateVendorDefinedEvent(CFAllocatorRef, uint64_t, uint32_t, uint32_t, uint32_t, const uint8_t *, CFIndex, uint32_t);
extern IOHIDEventSystemClientRef IOHIDEventSystemClientCreateWithType(CFAllocatorRef, int, CFDictionaryRef);
extern void IOHIDEventSystemClientDispatchEvent(IOHIDEventSystemClientRef, IOHIDEventRef);
extern CFDataRef IOCFSerialize(CFTypeRef, CFOptionFlags);

static void post(NSString *source, NSString *type, id value) {
    IOHIDEventSystemClientRef client = IOHIDEventSystemClientCreateWithType(NULL, 4, NULL);
    NSDictionary *payload = @{@"provider": @"com.apple.Virtualization.VirtualMachines", @"source": source, @"type": type, @"value": value};
    CFDataRef data = IOCFSerialize((__bridge CFTypeRef)payload, 1);
    IOHIDEventRef event = IOHIDEventCreateVendorDefinedEvent(NULL, mach_absolute_time(), 0xFF61, 0x5B, 0, CFDataGetBytePtr(data), CFDataGetLength(data), 0);
    IOHIDEventSystemClientDispatchEvent(client, event);
    CFRelease(event);
    CFRelease(data);
    usleep(20000);
}

int main(int argc, char **argv) {
    if (argc != 3) { fprintf(stderr, "usage: duo_helper angle <deg> | orient <name>\n"); return 64; }
    if (strcmp(argv[1], "angle") == 0) post(@"hinge-slider-control", @"range", @(atof(argv[2])));
    else if (strcmp(argv[1], "orient") == 0) post(@"orientation-picker-control", @"enum", @(argv[2]));
    else return 64;
    return 0;
}
