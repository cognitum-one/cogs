/*===============================================================================================================================
    Copyright  2013 Advanced Architectures

    All rights reserved
    Confidential Information
    Limited Distribution to Authorized Persons Only
    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

    Project Name         : NEWPORT
    Description          : 256-way priority encoder


================================================================================================================================
    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
================================================================================================================================
    Notes for Use and Synthesis

    NOT a low-power implementation!

==============================================================================================================================*/

module A2_priority256 (
   output wire [  7:0]  penc,
   output wire          flag,
   input  wire [255:0]  di
   );

wire  [63:0]   location;
wire  [15:0]   found;
wire  [ 3:0]   upper, lower;

genvar   i;
for (i=0; i<16; i=i+1) begin : A
   A2_priority16 i_L1 (
      .penc(location[4*i+:4]  ),
      .flag(   found[i]       ),
      .di  (      di[16*i+:16])
      );
   end

A2_priority16 i_L2 (
   .penc (upper[ 3:0]),
   .flag (flag       ),
   .di   (found[15:0])
   );

assign
   lower[3:0]  =  location >> (4*upper),
   penc [7:0]  = {upper[3:0],lower[3:0]};

endmodule

// Sub-module ===================================================================================================================

module A2_priority16 (
   output reg  [ 3:0]   penc,
   output reg           flag,
   input  wire [15:0]   di           // LSB "0" is highest priority
   );

integer  i;
always @* begin
   penc = 4'h0;
   flag = 1'b0;
   for (i=0; i<16; i=i+1)
      if (!flag)
         if (di[i]) begin
            flag  = 1'b1;
            penc  = i;
            end
   end

endmodule
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

//`define  verify_penc
`ifdef   verify_penc

module t;

wire [  7:0]   penc;
wire           flag;
reg  [255:0]   data;

integer  i;
A2_priority256  mut (
   .penc  (penc),
   .flag  (flag),
   .di    (data)
   );

initial begin

   data = (256'h1 << 256) - 1;

   for (i=0; i<257; i=i+1) begin
      #10;
      $display(" %h => %h,%h ", data,flag,penc);
      data = {data[254:0],1'b0};
      end

   $stop();
   $finish();
   end
endmodule

`endif //verify_penc

