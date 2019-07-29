CREATE TABLE queue (
	uid BIGINT unsigned AUTO_INCREMENT PRIMARY KEY,
	server INTEGER unsigned NOT NULL,
	message INTEGER unsigned NOT NULL,
	timestamp INT unsigned NOT NULL
);
