use sea_query::*;
use sea_query_sqlx::SqlxBinder;
use sqlx::types::Uuid;

use crate::idens::ai_conversation_idens::{
    AiChatIden, AiConversationsIden, AiWorkflowQuickUploadIden,
};
use crate::idens::user_idens::UsersIden;
use crate::query_params::ai_conversation_params::{
    ConversationErrorTarget, GetConversationsParams, GetConversationsSearchType,
    UpdateConversationErrorParams,
};

use super::DbQueryWithValues;

#[macros::named_query]
pub fn create_conversation(user_id: Uuid) -> DbQueryWithValues {
    Query::insert()
        .into_table(AiConversationsIden::Table)
        .columns([AiConversationsIden::UserId])
        .values_panic([user_id.into()])
        .returning(Query::returning().column(AiConversationsIden::Id))
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn create_chat(conversation_id: Uuid) -> DbQueryWithValues {
    Query::insert()
        .into_table(AiChatIden::Table)
        .columns([AiChatIden::ConversationId])
        .values_panic([conversation_id.into()])
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_owned_conversation_id(conversation_id: Uuid, user_id: Uuid) -> DbQueryWithValues {
    Query::select()
        .column(AiConversationsIden::Id)
        .from(AiConversationsIden::Table)
        .and_where(Expr::col(AiConversationsIden::Id).eq(conversation_id))
        .and_where(Expr::col(AiConversationsIden::UserId).eq(user_id))
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_conversations(params: GetConversationsParams) -> DbQueryWithValues {
    let mut query = Query::select();
    query
        .column(AiConversationsIden::Id)
        .column(AiConversationsIden::UserId)
        .column((AiConversationsIden::Table, AiConversationsIden::CreatedAt))
        .column((AiConversationsIden::Table, AiConversationsIden::UpdatedAt))
        .column((AiConversationsIden::Table, AiConversationsIden::LastError))
        .column((AiChatIden::Table, AiChatIden::Title))
        .expr_as(
            Expr::cust(
                "CASE WHEN ai_chat.title IS NULL THEN substring(ai_conversations.history FROM 1 FOR 16384) END",
            ),
            Alias::new("untitled_history_head"),
        )
        .from(AiConversationsIden::Table)
        .inner_join(
            AiChatIden::Table,
            Expr::col((AiChatIden::Table, AiChatIden::ConversationId))
                .equals((AiConversationsIden::Table, AiConversationsIden::Id)),
        )
        .and_where(Expr::col(AiConversationsIden::UserId).eq(params.user_id));
    match params.search_type {
        GetConversationsSearchType::ById(id) => {
            query.and_where(Expr::col(AiConversationsIden::Id).eq(id));
        }
        GetConversationsSearchType::All => {
            query.order_by(
                (AiConversationsIden::Table, AiConversationsIden::CreatedAt),
                Order::Desc,
            );
        }
    }
    if let Some(paging) = params.paging {
        query.limit(paging.count).offset(paging.start);
    }
    query.build_sqlx(PostgresQueryBuilder).into()
}

#[macros::named_query]
pub fn update_chat_title_if_null(conversation_id: Uuid, title: String) -> DbQueryWithValues {
    Query::update()
        .table(AiChatIden::Table)
        .value(AiChatIden::Title, title)
        .value(AiChatIden::UpdatedAt, Expr::cust("NOW()"))
        .and_where(Expr::col(AiChatIden::ConversationId).eq(conversation_id))
        .and_where(Expr::col(AiChatIden::Title).is_null())
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_chats_needing_titles(limit: u64) -> DbQueryWithValues {
    Query::select()
        .column((AiChatIden::Table, AiChatIden::ConversationId))
        .column(AiConversationsIden::UserId)
        .from(AiChatIden::Table)
        .inner_join(
            AiConversationsIden::Table,
            Expr::col((AiConversationsIden::Table, AiConversationsIden::Id))
                .equals((AiChatIden::Table, AiChatIden::ConversationId)),
        )
        .inner_join(
            UsersIden::Table,
            Expr::col((UsersIden::Table, UsersIden::Id))
                .equals((AiConversationsIden::Table, AiConversationsIden::UserId)),
        )
        .and_where(Expr::col((UsersIden::Table, UsersIden::AiFeaturesEnabled)).eq(true))
        .and_where(Expr::col(AiChatIden::Title).is_null())
        .and_where(Expr::cust("octet_length(ai_conversations.history) > 0"))
        .and_where(Expr::cust(
            "ai_conversations.updated_at < NOW() - INTERVAL '600 seconds'",
        ))
        .order_by(
            (AiConversationsIden::Table, AiConversationsIden::CreatedAt),
            Order::Asc,
        )
        .limit(limit)
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn get_conversation_history(conversation_id: Uuid, user_id: Uuid) -> DbQueryWithValues {
    Query::select()
        .column(AiConversationsIden::History)
        .from(AiConversationsIden::Table)
        .and_where(Expr::col(AiConversationsIden::Id).eq(conversation_id))
        .and_where(Expr::col(AiConversationsIden::UserId).eq(user_id))
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn append_conversation_history(
    conversation_id: Uuid,
    user_id: Uuid,
    expected_len: u64,
    bytes: Vec<u8>,
) -> DbQueryWithValues {
    Query::update()
        .table(AiConversationsIden::Table)
        .value(
            AiConversationsIden::History,
            Expr::cust_with_values("history || $1", [bytes]),
        )
        .value(AiConversationsIden::UpdatedAt, Expr::cust("NOW()"))
        .and_where(Expr::col(AiConversationsIden::Id).eq(conversation_id))
        .and_where(Expr::col(AiConversationsIden::UserId).eq(user_id))
        .and_where(Expr::cust_with_values(
            "octet_length(history) = $1",
            [expected_len as i64],
        ))
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn delete_chat(conversation_id: Uuid, user_id: Uuid) -> DbQueryWithValues {
    Query::delete()
        .from_table(AiConversationsIden::Table)
        .and_where(Expr::col(AiConversationsIden::Id).eq(conversation_id))
        .and_where(Expr::col(AiConversationsIden::UserId).eq(user_id))
        .and_where(
            Expr::col(AiConversationsIden::Id).in_subquery(
                Query::select()
                    .column(AiChatIden::ConversationId)
                    .from(AiChatIden::Table)
                    .take(),
            ),
        )
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn delete_conversations(conversation_ids: Vec<Uuid>, user_id: Uuid) -> DbQueryWithValues {
    Query::delete()
        .from_table(AiConversationsIden::Table)
        .and_where(Expr::col(AiConversationsIden::Id).is_in(conversation_ids))
        .and_where(Expr::col(AiConversationsIden::UserId).eq(user_id))
        .build_sqlx(PostgresQueryBuilder)
        .into()
}

#[macros::named_query]
pub fn update_conversation_error(params: UpdateConversationErrorParams) -> DbQueryWithValues {
    let mut query = Query::update();
    query
        .table(AiConversationsIden::Table)
        .value(AiConversationsIden::UpdatedAt, Expr::cust("NOW()"));

    match params.error {
        Some(error) => {
            query.value(AiConversationsIden::LastError, error);
        }
        None => {
            query.value(AiConversationsIden::LastError, Expr::cust("NULL"));
            // Skip rows already clear so a routine turn-start doesn't bump
            // updated_at on every conversation that never errored.
            query.and_where(Expr::col(AiConversationsIden::LastError).is_not_null());
        }
    }

    match params.target {
        ConversationErrorTarget::Conversation(conversation_id) => {
            query.and_where(Expr::col(AiConversationsIden::Id).eq(conversation_id));
        }
        ConversationErrorTarget::QuickUpload(quick_upload_id) => {
            query.and_where(
                Expr::col(AiConversationsIden::Id).in_subquery(
                    Query::select()
                        .column(AiWorkflowQuickUploadIden::ConversationId)
                        .from(AiWorkflowQuickUploadIden::Table)
                        .and_where(Expr::col(AiWorkflowQuickUploadIden::Id).eq(quick_upload_id))
                        .take(),
                ),
            );
        }
    }

    query.build_sqlx(PostgresQueryBuilder).into()
}
