# JMeter catalog generator

`gen-templates.groovy` runs inside JMeter and writes the XML JMeter itself saves for every element in its "Add" menus.
The app embeds the result as `crates/plan-core/data/templates.txt`, so a new element starts with exactly the settings the
JMeter GUI would give it, and the list of what can be added is JMeter's own.

## Regenerate

Needs Java and a JMeter install. From this folder (adjust the JMeter path):

```
java -Djava.awt.headless=true -jar <jmeter>/bin/ApacheJMeter.jar -n -t host.jmx -j out/gen.log -l out/gen.jtl ^
  -Jgen.script=<full path>/gen-templates.groovy -Jgen.out=<full path>/out/templates.txt
copy out\templates.txt ..\..\crates\plan-core\data\templates.txt
```

`host.jmx` is a one-sample plan that runs the script through a JSR223 sampler. The script ends with `System.exit(0)`
because some GUI classes start threads that would keep JMeter running.

The output has one block per menu entry:

```
@@@ BEGIN <menu id> | <gui class> | <test class> | <label> | <is a bean element>
<a complete .jmx document with the element>
@@@ END
```

and a `@@@ FAILED` line for any entry that could not be created (the generated file for 5.6.3 has none; a unit test
fails if one appears).

## Checking the result

`cargo test -p plan-core` checks that every entry loads, that JMeter's menu groups agree with the app's own
classification, and that the settings named by the hand-made forms exist in these defaults.
`crates/plan-core/tests/jmeter_load.rs` builds a plan with one of every element and loads it in real JMeter.

The data is JMeter 5.6.3 output (Apache-2.0); regenerate it when supporting another JMeter version.
