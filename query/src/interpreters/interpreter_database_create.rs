// Copyright 2021 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::sync::Arc;

use common_exception::ErrorCode;
use common_exception::Result;
use common_meta_types::GrantObject;
use common_meta_types::UserPrivilegeType;
use common_planners::CreateDatabasePlan;
use common_streams::DataBlockStream;
use common_streams::SendableDataBlockStream;
use common_tracing::tracing;

use crate::interpreters::Interpreter;
use crate::sessions::QueryContext;

#[derive(Debug)]
pub struct CreateDatabaseInterpreter {
    ctx: Arc<QueryContext>,
    plan: CreateDatabasePlan,
}

impl CreateDatabaseInterpreter {
    pub fn try_create(ctx: Arc<QueryContext>, plan: CreateDatabasePlan) -> Result<Self> {
        Ok(CreateDatabaseInterpreter { ctx, plan })
    }
}

#[async_trait::async_trait]
impl Interpreter for CreateDatabaseInterpreter {
    fn name(&self) -> &str {
        "CreateDatabaseInterpreter"
    }

    #[tracing::instrument(level = "debug", skip(self, _input_stream), fields(ctx.id = self.ctx.get_id().as_str()))]
    async fn execute(
        &self,
        _input_stream: Option<SendableDataBlockStream>,
    ) -> Result<SendableDataBlockStream> {
        self.ctx
            .get_current_session()
            .validate_privilege(&GrantObject::Global, UserPrivilegeType::Create)
            .await?;

        let tenant = self.plan.tenant.clone();
        let quota_api = self
            .ctx
            .get_user_manager()
            .get_tenant_quota_api_client(&tenant)?;
        let quota = quota_api.get_quota(None).await?.data;
        let catalog = self.ctx.get_catalog(&self.plan.catalog)?;
        let databases = catalog.list_databases(&tenant).await?;
        if quota.max_databases != 0 && databases.len() >= quota.max_databases as usize {
            return Err(ErrorCode::TenantQuotaExceeded(format!(
                "Max databases quota exceeded {}",
                quota.max_databases
            )));
        };
        catalog.create_database(self.plan.clone().into()).await?;

        Ok(Box::pin(DataBlockStream::create(
            self.plan.schema(),
            None,
            vec![],
        )))
    }
}
