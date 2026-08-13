use serde::Serialize;

use crate::models::Block;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ArtifactType {
    pub key: &'static str,
    pub label: &'static str,
    pub short_label: &'static str,
    pub description: &'static str,
    pub default_title: &'static str,
    pub template: &'static str,
    pub agent_instructions: &'static str,
}

pub const TYPES: &[ArtifactType] = &[
    ArtifactType {
        key: "prompt",
        label: "Prompt / 指令",
        short_label: "Prompt",
        description: "可直接交給 AI 執行的單次或通用指令。",
        default_title: "Reusable Prompt",
        template: r#"# 目標

<!-- 說明要完成的結果 -->

## 指令

1. 
2. 

## 輸入

<!-- 貼上或描述執行時會提供的資料 -->

## 限制

- 

## 期望輸出

<!-- 格式、語氣、長度與驗收條件 -->"#,
        agent_instructions: "這是可直接執行的 Prompt。先確認必要輸入是否齊全，再依限制與期望輸出完成工作；不要把它當成 Skill 或工具設定安裝。",
    },
    ArtifactType {
        key: "prompt_template",
        label: "Prompt Template / 提示模板",
        short_label: "Template",
        description: "含 {{變數}} 的可重複提示模板，適合不同任務套值。",
        default_title: "Prompt Template",
        template: r#"# 模板用途

<!-- 這份模板解決什麼問題 -->

## 變數

| 變數 | 必填 | 說明 | 範例 |
| --- | --- | --- | --- |
| `{{input}}` | 是 | 主要輸入 | 會議逐字稿 |
| `{{audience}}` | 否 | 目標讀者 | 專案團隊 |

## Prompt

請根據以下輸入完成任務：

{{input}}

目標讀者：{{audience}}

## 輸出要求

- 

## 使用範例

<!-- 示範一組變數與理想輸出 -->"#,
        agent_instructions: "這是 Prompt Template。先註冊或保存到目前 Agent 平台的 Prompt Template／模板區，辨識所有 `{{變數}}`；若必填值缺少，向使用者詢問，不要自行虛構。代入後再執行 Prompt，未提供的選填值可依模板語意省略。",
    },
    ArtifactType {
        key: "skill",
        label: "Skill / 專業技能",
        short_label: "Skill",
        description: "定義 Agent 何時啟用及如何執行一套可重複能力。",
        default_title: "Agent Skill",
        template: r#"# Skill 名稱

## 何時使用

<!-- 描述觸發條件，以及不應使用的情況 -->

## 前置條件

- 

## 執行流程

1. 
2. 
3. 

## 可用工具與資源

- 

## 品質檢查

- [ ] 

## 失敗與例外處理

<!-- 缺資料、工具不可用、風險過高時如何處理 -->"#,
        agent_instructions: "這是一份 Agent Skill，不是要直接回答的普通 Prompt。請先將下方內容安裝或寫入目前 Agent 平台的 Skill 目錄／Skill 設定（通常建立或更新 `SKILL.md`），完成安裝後再依「何時使用」條件載入。若平台不支援 Skill 安裝，請告知使用者並提供對應的手動安裝步驟，不要假裝已安裝。啟用後依序執行流程、遵守前置條件，完成前逐項做品質檢查；遇到例外時採用文件指定的處理方式。",
    },
    ArtifactType {
        key: "mcp_server",
        label: "MCP Server / 工具連線",
        short_label: "MCP",
        description: "記錄 MCP Server 的連線方式、能力、設定與使用界線。",
        default_title: "MCP Server Profile",
        template: r#"# Server 身分

- 名稱：
- 用途：
- 擁有者／文件：

## Transport 與連線

- Transport：`stdio` / `streamable-http`
- Command 或 URL：
- Arguments：

## 環境變數

<!-- 只寫變數名稱與取得方式，不要在 Vault 放入真正祕密 -->

| 變數 | 必填 | 說明 |
| --- | --- | --- |
| `EXAMPLE_TOKEN` | 是 | 由安全的 secrets manager 提供 |

## Tools / Resources / Prompts

| 名稱 | 類型 | 用途 | 重要參數 |
| --- | --- | --- | --- |
|  | tool |  |  |

## 安裝與驗證

1. 
2. 

## 安全界線

- 

## 常見錯誤

- "#,
        agent_instructions: "這是一份 MCP Server 資產，不是要直接執行的 Prompt。請先將下方內容加入目前 Agent 平台的 MCP／MCP Server 設定，依 transport、command／URL、arguments 與環境變數完成配置，再進行連線與 tools／resources／prompts 列舉驗證。祕密必須由安全的 secrets manager 或環境提供；未完成實際連線前，不可宣稱 MCP 已可用。若平台不支援 MCP，請告知使用者並提供手動設定方式。",
    },
    ArtifactType {
        key: "agent_profile",
        label: "Agent Profile / 角色設定",
        short_label: "Agent",
        description: "描述 Agent 的角色、目標、行為、界線與回應風格。",
        default_title: "Agent Profile",
        template: r#"# 角色

你是……

## 核心目標

- 

## 行為原則

- 

## 能力與工具

- 

## 不可做的事

- 

## 回應風格

- 語言：
- 詳細程度：
- 格式偏好：

## 需要確認的動作

- "#,
        agent_instructions: "這是 Agent Profile。請將內容加入目前 Agent 的 system／developer instructions 或角色設定，再開始任務；若與平台安全規則衝突，以平台規則為優先，若與使用者當下明確要求衝突，指出衝突並詢問。",
    },
    ArtifactType {
        key: "workflow",
        label: "Workflow / 工作流程",
        short_label: "Workflow",
        description: "可重跑的多步驟流程，包含分支、完成條件與通知。",
        default_title: "Reusable Workflow",
        template: r#"# 觸發條件

<!-- 何時執行這個 Workflow -->

## 所需輸入

- 

## 步驟

1. 
2. 
3. 

## 判斷與分支

- 如果……則……

## 完成條件

- [ ] 

## 產出

- 

## 完成後通知

- status：`completed` / `needs_input` / `failed`
- 通知時機與摘要："#,
        agent_instructions: "這是可註冊的 Workflow。請先加入目前 Agent 的 workflow／automation 設定，再按步驟執行並保留先後依賴；遇到分支時明確說明採用哪條路徑。只有完成條件全部成立才回報完成，否則依情況回報需要輸入或失敗。",
    },
    ArtifactType {
        key: "context_pack",
        label: "Context Pack / 背景知識",
        short_label: "Context",
        description: "為任務提供範圍、事實、術語、來源與時效資訊。",
        default_title: "Context Pack",
        template: r#"# 適用範圍

## 核心事實

- 

## 術語表

| 術語 | 定義 |
| --- | --- |
|  |  |

## 已知假設

- 

## 資料來源與更新時間

- 來源：
- 最後確認：

## 不包含／未知

- "#,
        agent_instructions: "這是 Context Pack。請先加入目前 Agent 的 context／knowledge 設定或本次工作階段的背景，再把內容當作任務背景而非永久真理。回答時遵守適用範圍，區分已知事實與假設；若內容可能過時或問題落在未知範圍，先驗證或清楚標示不確定性。",
    },
    ArtifactType {
        key: "preferences",
        label: "Preferences / 個人偏好",
        short_label: "Preferences",
        description: "攜帶使用者長期的溝通、工具與工作方式偏好。",
        default_title: "My Working Preferences",
        template: r#"# 溝通偏好

- 慣用語言：
- 回應長度：
- 語氣：

## 工作方式

- 

## 預設技術選擇

- 

## 常用工具

- 

## 避免事項

- 

## 偏好更新規則

<!-- 哪些只是一時選擇，哪些可視為長期偏好 -->"#,
        agent_instructions: "這是可攜的 Preferences。請先加入目前 Agent 的 user preferences／偏好設定，再在不違反當下要求與安全規則的前提下套用。使用者本次明確指定不同做法時，以本次要求為準；不要把臨時選擇擴張成長期偏好。",
    },
    ArtifactType {
        key: "tool_api",
        label: "Tool / API Contract",
        short_label: "Tool API",
        description: "記錄外部工具或 API 的使用契約、輸入輸出與錯誤處理。",
        default_title: "Tool or API Contract",
        template: r#"# 用途

## Base URL / Command

`https://example.com/api`

## 認證

<!-- 僅描述機制與 secret 名稱，不要存真正 credential -->

## 操作

### 操作名稱

- Method：`GET`
- Path：`/resource`
- 輸入：
- 成功輸出：
- 副作用：無／有（說明）

## 錯誤與重試

- 

## 使用範例

```json
{}
```"#,
        agent_instructions: "這是 Tool／API Contract。請先將它加入目前 Agent 的 tool／connector registry 或 API 設定，再依契約呼叫工具；在有副作用的操作前確認授權與目標。不可臆造未列出的 endpoint 或參數，遇到錯誤依重試規則處理，並避免把 credential 寫進輸出或 log。",
    },
    ArtifactType {
        key: "schema",
        label: "Schema / 結構化輸出",
        short_label: "Schema",
        description: "定義 Agent 輸出資料的欄位、型別、限制與範例。",
        default_title: "Structured Output Schema",
        template: r#"# Schema 用途

## 格式

`JSON`

## 定義

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["result"],
  "properties": {
    "result": {
      "type": "string",
      "description": ""
    }
  }
}
```

## 欄位規則

- 

## 合法範例

```json
{
  "result": "example"
}
```"#,
        agent_instructions: "這是輸出 Schema。請先加入目前 Agent 的 structured output／response schema 設定，之後輸出必須符合這份 Schema，不要加入未允許的欄位或 Markdown 包裝。產出前檢查必填欄位、型別、enum 與格式；若輸入不足以形成合法結果，先要求補充。",
    },
    ArtifactType {
        key: "evaluation_rubric",
        label: "Evaluation Rubric / 評分規則",
        short_label: "Rubric",
        description: "用一致的準則評估 Agent 產出並決定是否通過。",
        default_title: "Evaluation Rubric",
        template: r#"# 評估目標

## 評分準則

| 準則 | 權重 | 1 分 | 3 分 | 5 分 |
| --- | ---: | --- | --- | --- |
| 正確性 | 40% |  |  |  |
| 完整性 | 30% |  |  |  |
| 可用性 | 30% |  |  |  |

## 必要條件

- [ ] 

## 不合格條件

- 

## 通過門檻

- 加權總分至少：
- 所有必要條件成立：是"#,
        agent_instructions: "這是 Evaluation Rubric。請先加入目前 Agent 的 evaluation／評測設定，使用文件中的準則逐項評分，為每個分數提供可核對的證據，再計算加權結果。任何不合格條件成立時直接標示不通過，不可只給沒有理由的總分。",
    },
    ArtifactType {
        key: "safety_policy",
        label: "Safety Policy / 操作界線",
        short_label: "Policy",
        description: "定義允許、需確認與禁止的動作，以及敏感資料處理。",
        default_title: "Agent Safety Policy",
        template: r#"# 適用範圍

## 可直接執行

- 

## 執行前必須確認

- 

## 禁止執行

- 

## 敏感資料處理

- 

## 外部系統與副作用

- 

## 無法判定時

<!-- 停止、回報與升級方式 -->"#,
        agent_instructions: "這是一份 Safety Policy。請先加入目前 Agent 的 policy／guardrail／instructions 設定，再把它當作額外操作界線。先判定行為屬於可直接執行、必須確認或禁止；無法判定時停止有副作用的動作並詢問。此政策不能降低平台本身的安全要求。",
    },
];

pub fn find(key: &str) -> Option<&'static ArtifactType> {
    TYPES.iter().find(|item| item.key == key)
}

pub fn render_portable_pack(blocks: &[Block]) -> String {
    let mut output = String::from(
        "# CrossPrompt Portable Agent Pack\n\n\
## 給接收 Agent 的使用說明\n\n\
以下是具有明確型別的可攜資產。請依每一項的 `Type` 與「如何使用」處理內容，不要把所有項目都當成要立即執行的 Prompt。這份文字包的目標是讓你在目前平台完成安裝、註冊、設定或套用，而不是只把內容回顯給使用者。\n\n\
- 先閱讀每項的使用方式，再完成指定的安裝／加入設定／套用／執行／驗證動作；完成後回報實際結果。\n\
- 同時存在多項資產時，依原始順序組合；若規則衝突，指出衝突並詢問使用者。\n\
- `MCP Server` 與 `Tool / API Contract` 只描述設定，不代表工具已實際連線。\n\
- `{{variable}}` 是待代入值；缺少必要值時先詢問，不可虛構。\n\
- 不要要求使用者把密碼、API Key 或 token 貼入對話或不安全的儲存空間。\n\n",
    );

    for (index, block) in blocks.iter().enumerate() {
        let artifact_type = find(&block.block_type).unwrap_or(&TYPES[0]);
        if index > 0 {
            output.push_str("\n---\n\n");
        }
        output.push_str(&format!(
            "## {}\n\n**Type:** {} (`{}`)\n\n### 如何使用\n\n{}\n\n### 內容\n\n{}",
            block.title,
            artifact_type.label,
            artifact_type.key,
            artifact_type.agent_instructions,
            block.content
        ));
    }

    output
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn catalog_has_unique_complete_types() {
        let keys = TYPES.iter().map(|item| item.key).collect::<HashSet<_>>();
        assert_eq!(keys.len(), TYPES.len());
        assert!(TYPES.len() >= 10);
        for item in TYPES {
            assert!(!item.default_title.trim().is_empty());
            assert!(!item.template.trim().is_empty());
            assert!(!item.agent_instructions.trim().is_empty());
        }
        assert!(find("skill")
            .unwrap()
            .agent_instructions
            .contains("Skill 目錄"));
        assert!(find("mcp_server")
            .unwrap()
            .agent_instructions
            .contains("MCP／MCP Server 設定"));
    }
}
