//==============================================================================================================================
//
//   Copyright © 1996..2024 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name          : Library
//
//   Description           : One-Hot select Multiplxer.  Paramterized WIDTH and number of INPUTS
//
//===============================================================================================================================
//      THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//
//===============================================================================================================================
//   Prototype:    A2_mux #(8,32) i_XXX (.z(z[31:0]), .d(d[255:0]), .s(s[7:0]));
//===============================================================================================================================

`define V2

module A2_mux #(
   parameter   INPUTS   =   8,
   parameter   WIDTH    =  32
   )(
   output reg  [WIDTH-1       :0]   z,
   input  wire [INPUTS*WIDTH-1:0]   d,
   input  wire [INPUTS-1      :0]   s
   );

integer   i;

`ifdef  V2
always @*
   for (i=0; i<INPUTS; i=i+1)
      if (i==0) z =     {WIDTH{s[i]}} & d[i*WIDTH +: WIDTH];
      else      z = z | {WIDTH{s[i]}} & d[i*WIDTH +: WIDTH];
`elsif  V1
always @* begin
   z = {WIDTH{s[0]}} & d[WIDTH-1:0];
   for (i=1; i<INPUTS; i=i+1)
      z = z | {WIDTH{s[i]}} & d[i*WIDTH +: WIDTH];
   end
`else
always @* begin
   z = {WIDTH{1'b0}};
   for (i=0; i<INPUTS; i=i+1)
      z = z | {WIDTH{s[i]}} & d[i*WIDTH +: WIDTH];
`endif
endmodule


////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
