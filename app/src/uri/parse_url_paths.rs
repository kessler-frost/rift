use url::Url;

use crate::ChannelState;

#[derive(PartialEq, Debug)]
pub enum WarpWebLink {
    Session,
}

pub fn get_item_data_from_warp_link(url: &Url) -> Option<WarpWebLink> {
    if url.origin() == ChannelState::server_root_domain() {
        url.path_segments().and_then(|mut path_segments| {
            path_segments.next().and_then(|segment| match segment {
                "session" => Some(WarpWebLink::Session),
                _ => None,
            })
        })
    } else {
        None
    }
}
