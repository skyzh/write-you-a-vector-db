#include "execution/executors/vector_index_scan_executor.h"
#include <memory>
#include "common/macros.h"
#include "execution/execution_common.h"
#include "execution/executors/abstract_executor.h"
#include "fmt/core.h"

namespace bustub {
VectorIndexScanExecutor::VectorIndexScanExecutor(ExecutorContext *exec_ctx, const VectorIndexScanPlanNode *plan)
    : AbstractExecutor(exec_ctx), plan_(plan) {
  index_ = dynamic_cast<VectorIndex *>(exec_ctx->GetCatalog()->GetIndex(plan->GetIndexOid())->index_.get());
  table_ = exec_ctx->GetCatalog()->GetTable(plan->table_oid_)->table_.get();
}

void VectorIndexScanExecutor::Init() {
  Schema schema({});
  auto val = plan_->base_vector_->Evaluate(nullptr, schema);
  result_ = index_->ScanVectorKey(val.GetVector(), plan_->limit_);
  cnt_ = 0;
}

auto VectorIndexScanExecutor::Next(Tuple *tuple, RID *rid) -> bool {
  if (cnt_ >= result_.size()) {
    return false;
  }
  *rid = result_[cnt_];
  *tuple = table_->GetTuple(*rid).second;
  cnt_ += 1;
  return true;
}

}  // namespace bustub
