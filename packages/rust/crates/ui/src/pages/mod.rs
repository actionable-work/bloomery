pub mod api;
pub mod architecture;
pub mod home;
pub mod overrides;
pub mod overview;
pub mod profiles;
pub mod quickstart;

pub use api::*;
pub use architecture::*;
pub use home::*;
pub use overrides::*;
pub use overview::*;
pub use profiles::*;
pub use quickstart::*;

use topcoat::router::RouterBuilder;

/// Register all documentation and application pages onto a Topcoat router builder.
pub fn register_pages(builder: RouterBuilder) -> RouterBuilder {
    builder
        .page(home)
        .page(overview)
        .page(docs_overview)
        .page(quickstart)
        .page(profiles)
        .page(docs_profiles)
        .page(api)
        .page(docs_api)
        .page(overrides)
        .page(docs_overrides)
        .page(architecture)
}
