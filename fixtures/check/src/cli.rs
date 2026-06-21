use crate::analyzer::AnalyzerType;
use crate::web::repo::RepoType;

pub struct CliType;

// cli -> analyzer (same "app" group, downward) and cli -> web::repo
// (cross-group: no layer constraint).
fn _use(_a: AnalyzerType, _r: RepoType) {}
