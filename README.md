ltools
======

Command line tools to process LDIF data (LDIF - LDAP Data Interchange
Format).

For now this only includes `lget`, which extracts attribute values. If
more than one attribute is specified, `lget` will print a cartesian
product of the attributes in each entry as tab-separated values.  I
generally use it against the output of OpenLDAP's `ldapsearch`
command.

    $ lget --help
    lget 0.1.0

    USAGE:
	lget [OPTIONS] <ATTRIBUTES>...

    ARGS:
	<ATTRIBUTES>...    The attribute type name to get values of.

    OPTIONS:
	-0, --null-delimit    Terminate output values with null bytes (0x00) instead of newlines.
	-h, --help            Print help information
	-V, --version         Print version information