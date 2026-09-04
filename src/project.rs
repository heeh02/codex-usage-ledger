use std::cmp::Reverse;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::types::{AttributionConfidence, ProjectAttribution};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRecord {
    pub project_id: String,
    pub project_name: String,
    #[serde(default)]
    pub roots: Vec<PathBuf>,
    #[serde(default)]
    pub git_identities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualProjectAssignment {
    pub project_id: String,
    pub project_name: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectResolutionInput<'a> {
    pub manual: Option<&'a ManualProjectAssignment>,
    pub native_project_id: Option<&'a str>,
    pub cwd: Option<&'a Path>,
    pub git_identity: Option<&'a str>,
    pub parent: Option<&'a ProjectAttribution>,
}

/// Resolves attribution with a strict, deterministic priority:
/// manual > native project_id > longest root prefix > git identity > parent > unassigned.
pub fn resolve_project(
    input: ProjectResolutionInput<'_>,
    projects: &[ProjectRecord],
) -> ProjectAttribution {
    if let Some(manual) = input.manual {
        let project = projects
            .iter()
            .find(|project| project.project_id == manual.project_id);
        return ProjectAttribution {
            project_id: Some(manual.project_id.clone()),
            project_name: manual
                .project_name
                .clone()
                .or_else(|| project.map(|project| project.project_name.clone())),
            confidence: AttributionConfidence::Verified,
            method: "manual".to_owned(),
        };
    }

    if let Some(native_project_id) = input
        .native_project_id
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        let project = projects
            .iter()
            .find(|project| project.project_id == native_project_id);
        return ProjectAttribution {
            project_id: Some(native_project_id.to_owned()),
            project_name: project.map(|project| project.project_name.clone()),
            confidence: AttributionConfidence::Verified,
            method: "native_project_id".to_owned(),
        };
    }

    if let Some(cwd) = input.cwd {
        let cwd = lexical_normalize(cwd);
        let cwd_ref = &cwd;
        let mut matches = projects
            .iter()
            .flat_map(|project| {
                project.roots.iter().filter_map(move |root| {
                    let normalized_root = lexical_normalize(root);
                    cwd_ref.starts_with(&normalized_root).then_some((
                        normalized_root.components().count(),
                        normalized_root,
                        project,
                    ))
                })
            })
            .collect::<Vec<_>>();
        // Prefer the most specific root. Ties are deterministic and do not
        // depend on input iteration order.
        matches.sort_by_key(|(depth, root, project)| {
            (
                Reverse(*depth),
                root.to_string_lossy().into_owned(),
                project.project_id.clone(),
            )
        });
        if let Some((_, _, project)) = matches.first() {
            return attribution(
                project,
                AttributionConfidence::Inferred,
                "longest_root_prefix",
            );
        }
    }

    if let Some(git_identity) = input.git_identity.and_then(normalize_git_identity) {
        let mut matches = projects
            .iter()
            .filter(|project| {
                project
                    .git_identities
                    .iter()
                    .filter_map(|value| normalize_git_identity(value))
                    .any(|value| value == git_identity)
            })
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| left.project_id.cmp(&right.project_id));
        if let Some(project) = matches.first() {
            return attribution(project, AttributionConfidence::Inferred, "git_identity");
        }
    }

    if let Some(parent) = input.parent
        && parent.project_id.is_some()
    {
        return ProjectAttribution {
            project_id: parent.project_id.clone(),
            project_name: parent.project_name.clone(),
            confidence: AttributionConfidence::Inferred,
            method: "parent".to_owned(),
        };
    }

    ProjectAttribution {
        project_id: None,
        project_name: None,
        confidence: AttributionConfidence::Unknown,
        method: "unassigned".to_owned(),
    }
}

fn attribution(
    project: &ProjectRecord,
    confidence: AttributionConfidence,
    method: &str,
) -> ProjectAttribution {
    ProjectAttribution {
        project_id: Some(project.project_id.clone()),
        project_name: Some(project.project_name.clone()),
        confidence,
        method: method.to_owned(),
    }
}

/// Produces a comparison-only repository identity. Credentials, query strings,
/// fragments and a trailing `.git` are removed.
pub fn normalize_git_identity(value: &str) -> Option<String> {
    let mut value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Some((without_fragment, _)) = value.split_once('#') {
        value = without_fragment;
    }
    if let Some((without_query, _)) = value.split_once('?') {
        value = without_query;
    }

    let normalized = if let Some(rest) = value.strip_prefix("git@") {
        let (host, path) = rest.split_once(':')?;
        format!("{host}/{path}")
    } else if let Some((_, rest)) = value.split_once("://") {
        let rest = rest
            .rsplit_once('@')
            .map(|(_, value)| value)
            .unwrap_or(rest);
        rest.to_owned()
    } else {
        value.to_owned()
    };
    let normalized = normalized
        .trim_matches('/')
        .trim_end_matches(".git")
        .replace('\\', "/")
        .to_ascii_lowercase();
    (!normalized.is_empty()).then_some(normalized)
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                // Never pop a root/prefix. Relative leading `..` is retained so
                // it cannot accidentally match an unrelated absolute root.
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn project(id: &str, root: &str, git: &str) -> ProjectRecord {
        ProjectRecord {
            project_id: id.to_owned(),
            project_name: format!("Project {id}"),
            roots: vec![PathBuf::from(root)],
            git_identities: vec![git.to_owned()],
        }
    }

    #[test]
    fn strict_priority_is_manual_then_native_then_root_then_git_then_parent() {
        let projects = vec![
            project("root", "/work", "git@github.com:owner/root.git"),
            project(
                "nested",
                "/work/nested",
                "https://github.com/owner/nested.git",
            ),
            project("git", "/elsewhere", "ssh://git@github.com/owner/git.git"),
        ];
        let manual = ManualProjectAssignment {
            project_id: "manual".to_owned(),
            project_name: Some("Manual".to_owned()),
        };
        let parent = ProjectAttribution {
            project_id: Some("parent".to_owned()),
            project_name: Some("Parent".to_owned()),
            confidence: AttributionConfidence::Verified,
            method: "native_project_id".to_owned(),
        };

        let resolved = resolve_project(
            ProjectResolutionInput {
                manual: Some(&manual),
                native_project_id: Some("native"),
                cwd: Some(Path::new("/work/nested/src")),
                git_identity: Some("https://github.com/owner/git"),
                parent: Some(&parent),
            },
            &projects,
        );
        assert_eq!(resolved.project_id.as_deref(), Some("manual"));
        assert_eq!(resolved.method, "manual");

        let resolved = resolve_project(
            ProjectResolutionInput {
                native_project_id: Some("native"),
                cwd: Some(Path::new("/work/nested/src")),
                git_identity: Some("https://github.com/owner/git"),
                parent: Some(&parent),
                ..ProjectResolutionInput::default()
            },
            &projects,
        );
        assert_eq!(resolved.project_id.as_deref(), Some("native"));

        let resolved = resolve_project(
            ProjectResolutionInput {
                cwd: Some(Path::new("/work/nested/src")),
                git_identity: Some("https://github.com/owner/git"),
                parent: Some(&parent),
                ..ProjectResolutionInput::default()
            },
            &projects,
        );
        assert_eq!(resolved.project_id.as_deref(), Some("nested"));
        assert_eq!(resolved.method, "longest_root_prefix");

        let resolved = resolve_project(
            ProjectResolutionInput {
                cwd: Some(Path::new("/unmatched")),
                git_identity: Some("https://github.com/owner/git.git"),
                parent: Some(&parent),
                ..ProjectResolutionInput::default()
            },
            &projects,
        );
        assert_eq!(resolved.project_id.as_deref(), Some("git"));

        let resolved = resolve_project(
            ProjectResolutionInput {
                parent: Some(&parent),
                ..ProjectResolutionInput::default()
            },
            &projects,
        );
        assert_eq!(resolved.project_id.as_deref(), Some("parent"));
        assert_eq!(resolved.confidence, AttributionConfidence::Inferred);
    }

    #[test]
    fn unmatched_context_is_explicitly_unassigned() {
        let resolved = resolve_project(ProjectResolutionInput::default(), &[]);
        assert_eq!(resolved.project_id, None);
        assert_eq!(resolved.confidence, AttributionConfidence::Unknown);
        assert_eq!(resolved.method, "unassigned");
    }

    #[test]
    fn git_identity_normalization_removes_transport_credentials_and_suffix() {
        assert_eq!(
            normalize_git_identity("git@GitHub.com:Owner/Repo.git"),
            Some("github.com/owner/repo".to_owned())
        );
        assert_eq!(
            normalize_git_identity("ssh://git@github.com/Owner/Repo.git?x=1"),
            Some("github.com/owner/repo".to_owned())
        );
        assert_eq!(
            normalize_git_identity("https://token@github.com/Owner/Repo.git#main"),
            Some("github.com/owner/repo".to_owned())
        );
    }
}
