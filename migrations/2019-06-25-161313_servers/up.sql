CREATE TABLE servers (
	uid BIGINT unsigned AUTO_INCREMENT PRIMARY KEY,
	address VARCHAR(40) NOT NULL,
	version TINYINT unsigned NOT NULL,
	port SMALLINT unsigned NOT NULL
);
