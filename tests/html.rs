//! HTML rendering through `restir::render_html`.

mod common;

use common::html;

#[test]
fn escapes_html_characters() {
    assert_eq!(
        html("a & b < c > \"d\""),
        "<p>a &amp; b &lt; c &gt; &quot;d&quot;</p>\n"
    );
}

#[test]
fn resolves_backslash_escapes() {
    assert_eq!(html("\\*not em\\*"), "<p>*not em*</p>\n");
    assert_eq!(html("a\\\nb"), "<p>ab</p>\n");
    assert_eq!(
        html("これは\\ *強調*\\ です。"),
        "<p>これは<em>強調</em>です。</p>\n"
    );
}

#[test]
fn collapses_line_indentation() {
    assert_eq!(html("a\r\nb"), "<p>a\nb</p>\n");
    assert_eq!(html("- a\n  b"), "<ul>\n<li><p>a\nb</p>\n</li>\n</ul>\n");
}

#[test]
fn heading_levels_clamp_at_six() {
    let src = "A\n=\n\nB\n-\n\nC\n~\n\nD\n^\n\nE\n\"\n\nF\n'\n\nG\n`\n\nA2\n==\n";
    assert_eq!(
        html(src),
        "<h1>A</h1>\n<h2>B</h2>\n<h3>C</h3>\n<h4>D</h4>\n<h5>E</h5>\n<h6>F</h6>\n<h6>G</h6>\n<h1>A2</h1>\n"
    );
}

#[test]
fn lists() {
    assert_eq!(
        html("3. a\n4. b"),
        "<ol start=\"3\">\n<li><p>a</p>\n</li>\n<li><p>b</p>\n</li>\n</ol>\n"
    );
    assert_eq!(html("1. a"), "<ol>\n<li><p>a</p>\n</li>\n</ol>\n");
    assert_eq!(html("#. a"), "<ol>\n<li><p>a</p>\n</li>\n</ol>\n");
    assert_eq!(html("* a"), "<ul>\n<li><p>a</p>\n</li>\n</ul>\n");
}

#[test]
fn block_quote_transition_and_comment() {
    assert_eq!(html("  q"), "<blockquote>\n<p>q</p>\n</blockquote>\n");
    assert_eq!(
        html("a\n\n----\n\n.. c\n\nb"),
        "<p>a</p>\n<hr />\n<p>b</p>\n"
    );
}

#[test]
fn literal_blocks_are_dedented() {
    assert_eq!(
        html("x::\n\n    a\n      b\n\n    <c>"),
        "<p>x:</p>\n<pre>a\n  b\n\n&lt;c&gt;</pre>\n"
    );
    assert_eq!(html("::\n\n\tcode\n\t  more"), "<pre>code\n  more</pre>\n");
}

#[test]
fn inline_markup() {
    assert_eq!(
        html("*e* **s** ``<l>`` `c` ``a\\*b``"),
        "<p><em>e</em> <strong>s</strong> <code>&lt;l&gt;</code> <cite>c</cite> <code>a\\*b</code></p>\n"
    );
}

#[test]
fn roles() {
    assert_eq!(
        html(":sub:`x` :sup:`y` :t:`z` :code:`c` :emphasis:`e` :strong:`s` :foo:`f\\*`"),
        "<p><sub>x</sub> <sup>y</sup> <cite>z</cite> <code>c</code> <em>e</em> <strong>s</strong> <span class=\"role-foo\">f*</span></p>\n"
    );
}

#[test]
fn references() {
    assert_eq!(
        html("`My  Label`_ `x <http://a.b/?q=1&r=2>`_ `y`__"),
        "<p><a href=\"#my-label\">My  Label</a> <a href=\"http://a.b/?q=1&amp;r=2\">x</a> <a href=\"#y\">y</a></p>\n"
    );
}

#[test]
fn code_directives() {
    assert_eq!(
        html(".. code:: python\n   :number-lines:\n\n   def f():\n       return 1\n"),
        "<pre><code class=\"language-python\">def f():\n    return 1</code></pre>\n"
    );
    assert_eq!(
        html(".. sourcecode::\n\n  x < y"),
        "<pre><code>x &lt; y</code></pre>\n"
    );
}

#[test]
fn generic_directives_render_parsed_body() {
    assert_eq!(
        html(".. note::\n\n   Hello *world*.\n"),
        "<div class=\"directive directive-note\">\n<p>Hello <em>world</em>.</p>\n</div>\n"
    );
    assert_eq!(
        html(".. warning:: Be *careful*\n\n   body"),
        "<div class=\"directive directive-warning\">\n<p class=\"directive-arguments\">Be <em>careful</em></p>\n<p>body</p>\n</div>\n"
    );
}
