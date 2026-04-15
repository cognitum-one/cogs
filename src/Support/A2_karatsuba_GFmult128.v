//===============================================================================================================================
//
//    Copyright 2025 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//
//    Project Name   : COGNITUM
//
//    Description    : 128-bit Galois Field Multipler
//                     Karatsuba Hierarchical multiplier with a lower base of 8-bit polynomial multiplier
//                     Hierarchy: 8-bit, 16-bit, 32-bit, 64-bit, 128-bit (5 levels)
//
//
//===============================================================================================================================
//    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================

module A2_karatsuba_GFmult128 #(
   parameter M = 128
   )(
   output wire [254:0] z,        // Product output (255 bits)
   input  wire [127:0] x,        // First operand
   input  wire [127:0] y         // Second operand
   );

mult_128x128 multiplier (
   .a(x),
   .b(y),
   .p(z)
   );

endmodule

// 8-bit base multiplier (combinational) - uses partial products ================================================================

module mult_8x8 (
   output wire [15:0] p,
   input  wire [7:0]  a,
   input  wire [7:0]  b
   );

wire [15:0] pp [0:7];

genvar i;
   for (i=0; i<8; i=i+1) begin : gen_pp
      assign pp[i] = b[i] ? ({8'h0, a} << i) : 16'h0;
      end

assign
   p  = pp[0] ^ pp[1] ^ pp[2] ^ pp[3] ^ pp[4] ^ pp[5] ^ pp[6] ^ pp[7];

endmodule

// 16-bit Karatsuba multiplier (combinational) ==================================================================================

module mult_16x16 (
   output wire [31:0] p,
   input  wire [15:0] a,
   input  wire [15:0] b
   );

wire [7:0]
   a0     = a[7:0],
   b0     = b[7:0],
   a1     = a[15:8],
   b1     = b[15:8],
   a_sum  = a0 ^ a1,
   b_sum  = b0 ^ b1;

wire [15:0] z0, z1, z1_temp, z2;

mult_8x8 m0 (.a(a0),    .b(b0),    .p(z0));
mult_8x8 m1 (.a(a_sum), .b(b_sum), .p(z1_temp));
mult_8x8 m2 (.a(a1),    .b(b1),    .p(z2));

assign
   z1[15:0]    = z1_temp ^ z0 ^ z2, //JLPL - 10_21_25
   p[7:0]      = z0[7:0], //JLPL - 10_21_25
   p[15:8]     = z0[15:8] ^ z1[7:0], //JLPL - 10_21_25
   p[23:16]    = z1[15:8] ^ z2[7:0], //JLPL - 10_21_25
   p[31:24]    = z2[15:8];

endmodule

// 32-bit Karatsuba multiplier (combinational) ==================================================================================

module mult_32x32 (
   output wire [63:0] p,
   input  wire [31:0] a,
   input  wire [31:0] b
   );

wire [15:0]
   a0    = a[15:0],
   b0    = b[15:0],
   a1    = a[31:16],
   b1    = b[31:16],
   a_sum = a0 ^ a1,
   b_sum = b0 ^ b1;

wire [31:0] z0, z1, z1_temp, z2;

mult_16x16 m0 (.a(a0),    .b(b0),    .p(z0));
mult_16x16 m1 (.a(a_sum), .b(b_sum), .p(z1_temp));
mult_16x16 m2 (.a(a1),    .b(b1),    .p(z2));

assign
   z1[31:0]   = z1_temp ^ z0 ^ z2,
   p[15:0]    = z0[15:0],
   p[31:16]   = z0[31:16] ^ z1[15:0],
   p[47:32]   = z1[31:16] ^ z2[15:0],
   p[63:48]   = z2[31:16];

endmodule

// 64-bit Karatsuba multiplier (combinational) ==================================================================================


module mult_64x64 (
   output wire [127:0] p,
   input  wire [63:0]  a,
   input  wire [63:0]  b
   );

wire [31:0]
   a0    = a[31:0],
   b0    = b[31:0],
   a1    = a[63:32],
   b1    = b[63:32],
   a_sum = a0 ^ a1,
   b_sum = b0 ^ b1;

wire [63:0] z0, z1, z1_temp, z2;

mult_32x32 m0 (.a(a0),    .b(b0),    .p(z0));
mult_32x32 m1 (.a(a_sum), .b(b_sum), .p(z1_temp));
mult_32x32 m2 (.a(a1),    .b(b1),    .p(z2));

assign
   z1[63:0]    = z1_temp ^ z0 ^ z2,
   p[31:0]     = z0[31:0],
   p[63:32]    = z0[63:32] ^ z1[31:0],
   p[95:64]    = z1[63:32] ^ z2[31:0],
   p[127:96]   = z2[63:32];

endmodule

// 128-bit Karatsuba multiplier (combinational) =================================================================================

module mult_128x128 (
   output wire [254:0] p,
   input  wire [127:0] a,
   input  wire [127:0] b
   );

wire [63:0]
   a0    = a[63:0],
   b0    = b[63:0],
   a1    = a[127:64],
   b1    = b[127:64],
   a_sum = a0 ^ a1,
   b_sum = b0 ^ b1;

wire [127:0] z0, z1, z1_temp, z2;

mult_64x64 m0 (.a(a0),    .b(b0),    .p(z0));
mult_64x64 m1 (.a(a_sum), .b(b_sum), .p(z1_temp));
mult_64x64 m2 (.a(a1),    .b(b1),    .p(z2));

assign
   z1[127:0]   = z1_temp ^ z0 ^ z2,
   p[63:0]     = z0[63:0],
   p[127:64]   = z0[127:64] ^ z1[63:0],
   p[191:128]  = z1[127:64] ^ z2[63:0],
   p[254:192]  = z2[126:64];

endmodule

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
