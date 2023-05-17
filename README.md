ltools
======

Command line tools to process LDIF data (LDIF - LDAP Data Interchange
Format).

For now this only includes `lget`, which extracts attribute values from LDIF
entries. If more than one attribute is specified, `lget` will by default print
a cartesian product of the attributes in each entry as tab-separated values. It
additionally supports normal (non-regionalized) CSV output and JSON output.

    $ lget --help
    USAGE:
        lget [OPTIONS] <ATTRIBUTES>...

    ARGS:
        <ATTRIBUTES>...    The attribute type names to get values of.

    OPTIONS:
        -0, --null-delimit    Terminate output values with null bytes (0x00) instead of newlines.
        -c, --csv             Write values using the CSV format, including a header.
        -h, --help            Print help information
        -j, --json            Write specified attributes for each entry as a JSON object with string
                              array values.
        -V, --version         Print version information

Example usage:

    $ cat test.ldif | lget dn greeting
    cn=admin,dc=example,dc=com      Hello world!
    cn=foo,dc=example,dc=com        Dzień dobry!

`lget` does not differentiate between an LDIF entry's DN and attributes, except
that it will not print JSON objects for LDIF data that does not start with a DN (in
order to ignore things like version headers).

Cartesian product of entry attribute values is useful when an entry can have
more than one value of an attribute:

    $ cat test.ldif | lget dn member
    cn=group,dc=example,dc=com      cn=foo,dc=example,dc=com
    cn=group,dc=example,dc=com      cn=bar,dc=example,dc=com

The cartesian product will drop entries that lack the specified attributes,
unless you specify default values for the attributes using the `:-` syntax
borrowed from bash:

    $ cat test.ldif | lget dn manager:-no-manager
    cn=foo,dc=example,dc=com        no-manager
    cn=bar,dc=example,dc=com        cn=foo,dc=example,dc=com
    cn=baz,dc=example,dc=com        cn=foo,dc=example,dc=com

Attribute values can be base64-encoded by suffixing the attribute name with
`.base64`:

    $ cat test.ldif | lget dn control-characters.base64
    cn=bar,dc=example,dc=com        Zm9vCWJhcgBiYXoNCg==

In JSON output each entry is output as a JSON object on a single line, in which
each specified LDAP attribute is represented as an array of values.

    $ cat test.ldif | lget -j dn cn objectClass
    {"dn":["cn=admin,dc=example,dc=com"],"objectClass":["top"],"cn":["admin"]}
    {"cn":["foo"],"objectClass":["top","person"],"dn":["cn=foo,dc=example,dc=com"]}
    {"cn":["bar"],"objectClass":["top","person"],"dn":["cn=bar,dc=example,dc=com"]}
    {"objectClass":["top","person"],"dn":["cn=baz,dc=example,dc=com"],"cn":["baz"]}
    {"dn":["cn=group,dc=example,dc=com"],"cn":["group"],"objectClass":["top","groupOfNames"]}
