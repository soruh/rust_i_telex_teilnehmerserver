CREATE TABLE directory (
	uid BIGINT unsigned PRIMARY KEY,
	number int unsigned NOT NULL UNIQUE,
	name VARCHAR(40) NOT NULL,
	connection_type TINYINT unsigned NOT NULL,
	hostname VARCHAR(40),
	ipaddress INT unsigned,
	port SMALLINT unsigned NOT NULL,
	extension SMALLINT unsigned NOT NULL,
	pin SMALLINT unsigned NOT NULL,
	disabled BOOLEAN NOT NULL,
	timestamp INT unsigned NOT NULL,
	changed BOOLEAN NOT NULL
);
