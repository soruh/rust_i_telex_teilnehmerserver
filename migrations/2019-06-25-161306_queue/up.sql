CREATE TABLE queue (
	uid BIGINT unsigned PRIMARY KEY,
	server INTEGER unsigned NOT NULL,
	message INTEGER unsigned NOT NULL,
	timestamp INT unsigned NOT NULL
);
