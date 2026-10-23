// Run inside JMeter (see README.md). For every entry in JMeter's own "Add" menus, creates the element the
// way the GUI does and saves it the way JMeter saves plans, so the app can offer the same defaults.
//
// Output (property gen.out): blocks of
//   @@@ BEGIN <menu category> | <gui class> | <test class> | <label> | <testbean true/false>
//   <the element as JMeter saves it, a complete .jmx document>
//   @@@ END
// and, for failures,
//   @@@ FAILED <gui class> | <reason>

import org.apache.jmeter.gui.util.MenuFactory
import org.apache.jmeter.save.SaveService
import org.apache.jmeter.testbeans.TestBean
import org.apache.jmeter.testbeans.gui.TestBeanGUI
import org.apache.jorphan.collections.HashTree

def outPath = props.get('gen.out')
def sb = new StringBuilder()
def skip = [] as Set

def menuMap = MenuFactory.getMenuMap()
def seen = [] as Set
menuMap.each { category, infos ->
    infos.each { info ->
        def cn = info.getClassName()
        if (cn == null) {
            return // a separator
        }
        def key = category + '|' + cn
        if (!seen.add(key)) {
            return
        }
        try {
            if (cn.startsWith('javax.swing')) {
                return // a menu separator
            }
            // Create the element the way GuiPackage.createTestElement(String) does.
            def c = Class.forName(cn)
            def comp = TestBean.isAssignableFrom(c) ? new TestBeanGUI(c) : c.getDeclaredConstructor().newInstance()
            comp.clearGui()
            def el = comp.createTestElement()
            def tree = new HashTree()
            tree.add(el)
            def baos = new ByteArrayOutputStream()
            SaveService.saveTree(tree, baos)
            def isBean = TestBean.isAssignableFrom(Class.forName(cn))
            sb << "@@@ BEGIN ${category} | ${cn} | ${el.getClass().getName()} | ${comp.getStaticLabel()} | ${isBean}\n"
            sb << baos.toString('UTF-8')
            if (!sb.toString().endsWith('\n')) {
                sb << '\n'
            }
            sb << "@@@ END\n"
        } catch (Throwable t) {
            def where = t.getStackTrace().take(7).collect { it.toString() }.join(' <- ')
            sb << "@@@ FAILED ${cn} | ${t.getClass().getName()}: ${t.getMessage()} @ ${where}\n"
        }
    }
}
new File(outPath).setText(sb.toString(), 'UTF-8')
log.info("catalog written to ${outPath}")
System.exit(0) // a GUI class may have started a thread that would keep JMeter alive
