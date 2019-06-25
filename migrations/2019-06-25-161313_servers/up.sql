CREATE TABLE servers (
	uid BIGINT unsigned PRIMARY KEY,
	address VARCHAR(40) NOT NULL,
	version TINYINT unsigned NOT NULL,
	port SMALLINT unsigned NOT NULL
);
