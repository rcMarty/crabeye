DO
$$
    DECLARE
        _sql text;
    BEGIN
        FOR _sql IN (SELECT 'DROP INDEX "' || n.nspname || '"."' || i.relname || '";'
                     FROM pg_class i
                              JOIN pg_index ix ON i.oid = ix.indexrelid
                              JOIN pg_namespace n ON i.relnamespace = n.oid
                     WHERE i.relkind = 'i'
                       AND n.nspname NOT IN ('pg_catalog', 'pg_toast', 'information_schema')
                       AND ix.indisprimary = false
                       AND ix.indisunique = false)
            LOOP

                EXECUTE _sql;
            END LOOP;
    END
$$;