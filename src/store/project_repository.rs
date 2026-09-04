use super::*;

impl LedgerStore {
    pub fn upsert_project(
        &mut self,
        project: &ProjectRecord,
        updated_at: DateTime<Utc>,
    ) -> StoreResult<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO projects(project_id, project_name, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(project_id) DO UPDATE SET
                 project_name = excluded.project_name,
                 updated_at = excluded.updated_at",
            params![
                project.project_id,
                project.project_name,
                timestamp(updated_at)
            ],
        )?;
        transaction.execute(
            "DELETE FROM project_roots WHERE project_id = ?1",
            params![project.project_id],
        )?;
        transaction.execute(
            "DELETE FROM project_git_identities WHERE project_id = ?1",
            params![project.project_id],
        )?;
        for root in &project.roots {
            transaction.execute(
                "INSERT OR IGNORE INTO project_roots(project_id, root) VALUES (?1, ?2)",
                params![project.project_id, root.to_string_lossy()],
            )?;
        }
        for identity in &project.git_identities {
            let Some(identity) = normalize_git_identity(identity) else {
                continue;
            };
            transaction.execute(
                "INSERT OR IGNORE INTO project_git_identities(project_id, git_identity)
                 VALUES (?1, ?2)",
                params![project.project_id, identity],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn list_projects(&self) -> StoreResult<Vec<ProjectRecord>> {
        let mut statement = self
            .connection
            .prepare("SELECT project_id, project_name FROM projects ORDER BY project_id")?;
        let base = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut projects = Vec::with_capacity(base.len());
        for (project_id, project_name) in base {
            let roots = query_text_children(
                &self.connection,
                "SELECT root FROM project_roots WHERE project_id = ?1 ORDER BY root",
                &project_id,
            )?
            .into_iter()
            .map(Into::into)
            .collect();
            let git_identities = query_text_children(
                &self.connection,
                "SELECT git_identity FROM project_git_identities WHERE project_id = ?1 ORDER BY git_identity",
                &project_id,
            )?;
            projects.push(ProjectRecord {
                project_id,
                project_name,
                roots,
                git_identities,
            });
        }
        Ok(projects)
    }

    pub fn upsert_manual_assignment(
        &mut self,
        assignment_key: &str,
        assignment: &ManualProjectAssignment,
        updated_at: DateTime<Utc>,
    ) -> StoreResult<()> {
        self.connection.execute(
            "INSERT INTO manual_assignments(
                 assignment_key, project_id, project_name_override, updated_at
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(assignment_key) DO UPDATE SET
                 project_id = excluded.project_id,
                 project_name_override = excluded.project_name_override,
                 updated_at = excluded.updated_at",
            params![
                assignment_key,
                assignment.project_id,
                assignment.project_name,
                timestamp(updated_at),
            ],
        )?;
        Ok(())
    }

    pub fn get_manual_assignment(
        &self,
        assignment_key: &str,
    ) -> StoreResult<Option<StoredManualAssignment>> {
        self.connection
            .query_row(
                "SELECT assignment_key, project_id, project_name_override, updated_at
                 FROM manual_assignments WHERE assignment_key = ?1",
                params![assignment_key],
                |row| {
                    let updated_at: String = row.get(3)?;
                    Ok(StoredManualAssignment {
                        assignment_key: row.get(0)?,
                        assignment: ManualProjectAssignment {
                            project_id: row.get(1)?,
                            project_name: row.get(2)?,
                        },
                        updated_at: parse_timestamp_column(updated_at, 3)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
    }
}
