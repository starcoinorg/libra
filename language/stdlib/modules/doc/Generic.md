
<a name="0x1_Generic"></a>

# Module `0x1::Generic`

### Table of Contents

-  [Function `type_of`](#0x1_Generic_type_of)



<a name="0x1_Generic_type_of"></a>

## Function `type_of`

Return module address, module name, and type name of
<code>E</code>.


<pre><code><b>public</b> <b>fun</b> <a href="#0x1_Generic_type_of">type_of</a>&lt;E&gt;(): (address, vector&lt;u8&gt;, vector&lt;u8&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>native</b> <b>public</b> <b>fun</b> <a href="#0x1_Generic_type_of">type_of</a>&lt;E&gt;(): (address, vector&lt;u8&gt;, vector&lt;u8&gt;);
</code></pre>



</details>
