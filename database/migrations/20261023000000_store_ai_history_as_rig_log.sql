
DELETE FROM ai_conversations c
WHERE EXISTS (SELECT 1 FROM ai_chat WHERE conversation_id = c.id)
   OR EXISTS (SELECT 1 FROM ai_workflow_quick_upload WHERE conversation_id = c.id);

DROP TABLE ai_messages;

ALTER TABLE ai_conversations ADD COLUMN history BYTEA NOT NULL DEFAULT ''::bytea;
