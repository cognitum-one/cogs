/*===============================================================================================================================
    Copyright  2013 Advanced Architectures

    All rights reserved
    Confidential Information
    Limited Distribution to Authorized Persons Only
    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

    Project Name         : A2P CPU
    Description          : A2P Version 3  -- Parameterized Priority Encoder

                           Uses log N logic levels to derive the priority encoded value of N inputs
                                   2

    Author               : RTT
    Version              : 1.0

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
`define RTL
module A2_priority #(
   parameter   WIDTH       =  16,
   parameter   LG2_WIDTH   =   4   // `LG2(WIDTH)
   )(
   output wire [LG2_WIDTH-1:0]   penc,
   output wire                   flag,
   input  wire [WIDTH-1    :0]   di           // LSB "0" is highest priority
   );

localparam  MOD_WIDTH   = 4*(WIDTH%4>0?(WIDTH/4)+1:WIDTH/4);          // Works in groups of 4 so pad as necessary

wire  [MOD_WIDTH-1    :0]   any [0:LG2_WIDTH];                        // any bit set -- OR tree
wire  [MOD_WIDTH/2-1  :0]   val [1:LG2_WIDTH];                        // values  per stage
wire  [MOD_WIDTH-2    :0]   enc [2:LG2_WIDTH][0:(2**LG2_WIDTH)/4-1];  // encoding per stage

`ifdef   RTL

integer  i;
reg   [LG2_WIDTH-1:0] loc;
reg      found;
always @* begin
   found = 1'b0;
   for (i=0; i<WIDTH; i=i+1)
      if (!found)
         if (di[i]) begin
            found = 1'b1;
            loc   = i;
//            i     = WIDTH;
            end
   end
assign
   penc  =  loc,
   flag  =  found;
`else
genvar   i,j,k;

assign   any[0][MOD_WIDTH-1:0] = MOD_WIDTH-WIDTH == 0 ? di[WIDTH-1:0] : {{MOD_WIDTH-WIDTH{1'b 0}},di[WIDTH-1:0]};

for (j=0; j<LG2_WIDTH; j=j+1) begin : a
   for (i=0; i<(MOD_WIDTH/(2**j)); i=i+2) begin : b
      assign   any[j+1][i/2]  =  any[j][i]  | any[j][i+1];
      assign   val[j+1][i/2]  = ~any[j][i];
      if (j>0) for (k=0; k<j;  k=k+1) begin : c
         if (k<(j-1))
            assign   enc[j+1][i/2][k] =  any[j][i]  ? enc[j][i][k] : enc[j][i+1][k];
         else
            assign   enc[j+1][i/2][k] =  any[j][i]  ? val[j][i]    : val[j][i+1];
         end
      end
   end
`endif
// Output =============================================

assign   flag                 =  any[LG2_WIDTH][0];
assign   penc[LG2_WIDTH-1:0]  = {val[LG2_WIDTH][0], enc[LG2_WIDTH][0][LG2_WIDTH-2:0]};

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

parameter   WIDTH       = 256;
parameter   LG2_WIDTH   =   8;
parameter   MOD_WIDTH   = 4*(WIDTH%4>0?(WIDTH/4)+1:WIDTH/4);

wire [LG2_WIDTH-1 :0]   penc;
wire                    flag;
reg  [WIDTH-1     :0]   data;
integer  i;

A2_priority  #(
   .WIDTH (WIDTH)
   ) mut (
   .penc  (penc),
   .flag  (flag),
   .di    (data)
   );

initial begin
   $display("  WIDTH       = %d", WIDTH);
   $display("  MOD_WIDTH   = %d", MOD_WIDTH);

   data = (256'h1 << 256) - 1;

   for (i=0; i<WIDTH; i=i+1) begin
      #10;
      $display(" %h => %h,%h ", data,flag,penc);
      data = {data[254:0],1'b0};
      end

   $stop();
   $finish();
   end
endmodule

`endif //verify_penc

