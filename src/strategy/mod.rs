pub mod allocator;
pub mod executor;
pub mod precision;
pub mod rotator;
pub mod scanner;
pub mod trigger;

pub use allocator::{CapitalAllocator, DynamicAllocationDecision};
pub use executor::TwoLegExecutor;
pub use precision::LotPrecisionMatcher;
pub use rotator::{OpportunityRotator, SwapRecommendation};
pub use scanner::ArbitrageScanner;
#[allow(unused_imports)]
pub use trigger::{ProfitTriggerEngine, TriggerDecision};
