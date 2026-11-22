//! Versioned dave answers and the single digest-bound polkit invocation.
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, de::DeserializeOwned};
use std::{
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Index {
    Fresh,
    Cached,
    None,
}
impl Index {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fresh => "Fresh repository index",
            Self::Cached => "Cached repository index",
            Self::None => "No repository index · waiting for dave-refresh.timer",
        }
    }
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Install,
    Remove,
    Upgrade,
}
impl Operation {
    pub fn verb(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Remove => "remove",
            Self::Upgrade => "upgrade",
        }
    }
}
#[derive(Clone, Deserialize)]
pub struct Listed {
    pub name: String,
    pub version: String,
    pub requested: bool,
}
#[derive(Deserialize)]
struct List {
    packages: Vec<Listed>,
}
#[derive(Clone, Deserialize)]
pub struct Found {
    pub name: String,
    pub version: String,
    pub description: String,
    pub installed: Option<String>,
}
#[derive(Deserialize)]
pub struct Search {
    pub index: Index,
    pub packages: Vec<Found>,
}
#[derive(Deserialize)]
pub struct Installed {
    pub version: String,
    pub reason: String,
    pub needed_by: Option<String>,
    pub size: u64,
    pub files: u64,
}
#[derive(Deserialize)]
pub struct Available {
    pub version: String,
    pub repository: String,
    pub download_size: u64,
    pub installed_size: Option<u64>,
}
#[derive(Deserialize)]
pub struct Info {
    pub name: String,
    pub description: String,
    pub license: String,
    pub installed: Option<Installed>,
    pub available: Option<Available>,
    pub depends: Vec<String>,
    pub required_by: Vec<String>,
}
#[derive(Deserialize)]
pub struct Change {
    pub name: String,
    pub version: String,
    pub from: Option<String>,
    pub named: bool,
    pub requested: bool,
}
#[derive(Deserialize)]
pub struct Prepared {
    pub summary: String,
    pub install: usize,
    pub remove: usize,
}
#[derive(Deserialize)]
pub struct Plan {
    pub operation: Operation,
    pub index: Index,
    pub digest: Option<String>,
    pub install: Vec<Change>,
    pub upgrade: Vec<Change>,
    pub remove: Vec<Change>,
    pub download_size: u64,
    pub disk_change: Option<i64>,
    pub prepared: Option<Prepared>,
}
#[derive(Deserialize)]
pub struct Deployment {
    pub id: u32,
    pub health: Option<String>,
    pub pinned: bool,
    pub reason: String,
}
#[derive(Deserialize)]
pub struct LastUpgrade {
    pub outcome: String,
    pub deployment: Option<u32>,
    pub message: String,
}
#[derive(Deserialize)]
pub struct Status {
    pub revision: String,
    pub managed: bool,
    pub complete: bool,
    pub running: Option<u32>,
    pub next_boot: Option<u32>,
    pub deployments: Vec<Deployment>,
    pub prepared_upgrade: Option<Prepared>,
    pub last_upgrade: Option<LastUpgrade>,
}
#[derive(Deserialize)]
struct Receipt {
    operation: Operation,
    digest: Option<String>,
    rollback_point: Option<u32>,
}
#[derive(Deserialize)]
struct ErrorDoc {
    message: String,
}

fn decode<T: DeserializeOwned>(bytes: &[u8], schema: &str) -> Result<T> {
    let doc: serde_json::Value =
        serde_json::from_slice(bytes).context("Unreadable JSON from dave")?;
    let actual = doc
        .get("schema")
        .and_then(|s| s.as_str())
        .context("Missing dave schema")?;
    ensure!(actual == schema, "Expected {schema}, received {actual}.");
    serde_json::from_value(doc).context("Incomplete or invalid dave document")
}

fn valid_name(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() <= 128
        && b.first().is_some_and(u8::is_ascii_alphanumeric)
        && b.iter()
            .all(|c| c.is_ascii_alphanumeric() || b"-_+.".contains(c))
}
fn validate_name(name: &str) -> Result<()> {
    ensure!(valid_name(name), "Invalid dave package name: {name}");
    Ok(())
}

#[derive(Clone)]
pub struct Request {
    operation: Operation,
    names: Vec<String>,
}
impl Request {
    pub fn package(op: Operation, name: &str) -> Result<Self> {
        validate_name(name)?;
        ensure!(op != Operation::Upgrade, "Use the system upgrade action");
        Ok(Self {
            operation: op,
            names: vec![name.into()],
        })
    }
    pub fn upgrade() -> Self {
        Self {
            operation: Operation::Upgrade,
            names: vec![],
        }
    }
    pub fn operation(&self) -> Operation {
        self.operation
    }
    pub fn title(&self) -> String {
        match self.operation {
            Operation::Upgrade => "Upgrade system packages".into(),
            Operation::Install => format!("Install {}", self.names.join(", ")),
            Operation::Remove => format!("Remove {}", self.names.join(", ")),
        }
    }
}
pub struct Review {
    request: Request,
    plan: Plan,
}
impl Review {
    pub fn request(&self) -> &Request {
        &self.request
    }
    pub fn plan(&self) -> &Plan {
        &self.plan
    }
    fn new(request: Request, plan: Plan) -> Result<Self> {
        ensure!(
            request.operation == plan.operation,
            "dave returned a plan for another operation."
        );
        let empty = plan.install.is_empty() && plan.upgrade.is_empty() && plan.remove.is_empty();
        ensure!(
            plan.digest.is_none() == empty,
            "Inconsistent dave plan digest."
        );
        if let Some(d) = &plan.digest {
            ensure!(
                d.strip_prefix("sha256:")
                    .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())),
                "Invalid dave plan digest."
            );
        }
        for c in plan.install.iter().chain(&plan.upgrade).chain(&plan.remove) {
            validate_name(&c.name)?;
            ensure!(!c.version.is_empty(), "Missing planned version.");
        }
        ensure!(
            plan.upgrade
                .iter()
                .all(|c| c.from.as_ref().is_some_and(|v| !v.is_empty())),
            "Missing previous version in upgrade plan."
        );
        Ok(Self { request, plan })
    }
}
pub enum Applied {
    Success { rollback: Option<u32> },
    Dismissed,
    Denied,
    Refused(String),
}
#[derive(Clone)]
pub struct Dave {
    dave: PathBuf,
    pkexec: PathBuf,
}
impl Dave {
    pub fn system() -> Self {
        Self::with_programs("/usr/bin/dave".into(), "/usr/bin/pkexec".into())
    }
    pub fn with_programs(dave: PathBuf, pkexec: PathBuf) -> Self {
        Self { dave, pkexec }
    }

    fn command(program: &Path, args: &[&str]) -> Result<Output> {
        Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .with_context(|| format!("Could not run {}", program.display()))
    }
    fn query<T: DeserializeOwned>(&self, args: &[&str], schema: &str) -> Result<T> {
        let out = Self::command(&self.dave, args)?;
        if out.status.success() {
            return decode(&out.stdout, schema);
        }
        if out.status.code() == Some(1) {
            bail!(
                "{}",
                decode::<ErrorDoc>(&out.stdout, "dave.error/1")?.message
            );
        }
        bail!("dave exited with {}.", out.status)
    }
    pub fn list(&self) -> Result<Vec<Listed>> {
        Ok(self
            .query::<List>(&["list", "--json"], "dave.list/1")?
            .packages)
    }
    pub fn search(&self, terms: &str) -> Result<Search> {
        let mut args = vec!["search", "--json", "--"];
        args.extend(terms.split_whitespace());
        ensure!(args.len() > 3, "Enter a package name or search terms.");
        self.query(&args, "dave.search/1")
    }
    pub fn info(&self, name: &str) -> Result<Info> {
        validate_name(name)?;
        let info: Info = self.query(&["info", "--json", name], "dave.info/1")?;
        ensure!(
            info.name == name,
            "dave returned details for another package."
        );
        Ok(info)
    }
    pub fn status(&self) -> Result<Status> {
        self.query(&["status", "--json"], "dave.status/1")
    }
    pub fn preview(&self, request: Request) -> Result<Review> {
        let mut args = vec![request.operation.verb(), "--dry-run", "--json"];
        args.extend(request.names.iter().map(String::as_str));
        let plan = self.query(&args, "dave.plan/1")?;
        Review::new(request, plan)
    }
    /// Consume the reviewed plan. The caller must get a new review before retrying.
    pub fn apply(&self, review: Review) -> Result<Applied> {
        let digest = review
            .plan
            .digest
            .context("There are no changes to apply.")?;
        let mut args = vec![
            "/usr/bin/dave",
            review.request.operation.verb(),
            "-y",
            "--json",
            "--expect",
            &digest,
        ];
        args.extend(review.request.names.iter().map(String::as_str));
        let out = Self::command(&self.pkexec, &args)?;
        match out.status.code() {
            Some(0) => {
                let r: Receipt = decode(&out.stdout, "dave.result/1")?;
                ensure!(
                    r.operation == review.request.operation && r.digest.as_ref() == Some(&digest),
                    "Cannot verify the receipt for this change. Refresh package state."
                );
                Ok(Applied::Success {
                    rollback: r.rollback_point,
                })
            }
            Some(126) => Ok(Applied::Dismissed),
            Some(127) => Ok(Applied::Denied),
            Some(1) => Ok(Applied::Refused(
                decode::<ErrorDoc>(&out.stdout, "dave.error/1")?.message,
            )),
            _ => bail!(
                "Package authorization process exited with {}. Refresh package state.",
                out.status
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn read_dave_cli_documents_and_validate_reviews() {
        let docs: serde_json::Value =
            serde_json::from_str(include_str!("../../../../tests/packages/dave-v1.json")).unwrap();
        let bytes = |key: &str| serde_json::to_vec(&docs[key]).unwrap();
        let installed: List = decode(&bytes("installed"), "dave.list/1").unwrap();
        assert_eq!(installed.packages[0].name, "tree");
        let info: Info = decode(&bytes("info"), "dave.info/1").unwrap();
        assert_eq!(info.depends, ["libgreet >=1.1"]);
        let results: Search = decode(&bytes("search"), "dave.search/1").unwrap();
        assert_eq!(results.index, Index::Fresh);
        assert!(results.packages.iter().any(|p| p.name == "hello"));
        let status: Status = decode(&bytes("status"), "dave.status/1").unwrap();
        assert!(status.complete && !status.managed);
        let err: ErrorDoc = decode(&bytes("refusal"), "dave.error/1").unwrap();
        assert!(err.message.contains("hello"));
        for req in [
            Request::package(Operation::Install, "hello").unwrap(),
            Request::package(Operation::Remove, "tree").unwrap(),
            Request::upgrade(),
        ] {
            let doc = bytes(req.operation.verb());
            let plan: Plan = decode(&doc, "dave.plan/1").unwrap();
            let empty = req.operation == Operation::Upgrade;
            assert_eq!(Review::new(req, plan).unwrap().plan.digest.is_none(), empty);
        }
        let mut plan: Plan = decode(&bytes("install"), "dave.plan/1").unwrap();
        plan.digest = Some("invalid".into());
        assert!(Review::new(Request::package(Operation::Install, "hello").unwrap(), plan).is_err());
    }

    #[test]
    fn additive_fields_and_failures() {
        let list:List=decode(br#"{"schema":"dave.list/1","extra":1,"packages":[{"name":"ink","version":"1","requested":true,"future":12}]}"#,"dave.list/1").unwrap();
        assert_eq!(list.packages[0].name, "ink");
        assert_eq!(
            decode::<ErrorDoc>(
                br#"{"schema":"dave.error/1","message":"No index."}"#,
                "dave.error/1"
            )
            .unwrap()
            .message,
            "No index."
        );
        for invalid in [
            r#"{"schema":"dave.list/2","packages":[]}"#,
            r#"{"schema":"dave.list/1"}"#,
            "{}{}",
        ] {
            assert!(decode::<List>(invalid.as_bytes(), "dave.list/1").is_err());
        }
        for bad in ["--root", "../name", "", "x y", "a;b"] {
            assert!(Request::package(Operation::Remove, bad).is_err());
        }
        assert!(Request::package(Operation::Install, "libfoo++-2.1_x").is_ok());
    }
}
