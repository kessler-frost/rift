// Our class for handling NSServices messages.
@interface RiftServicesProvider : NSObject
@end

// Functions implemented in Rust.
id rift_services_provider_custom_url_scheme();
void rift_app_open_urls(id app, id urls);
