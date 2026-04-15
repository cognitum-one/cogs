/* ******************************************************************************************************************************

   Copyright © 2022..2024 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         :  A2F Floating Point for A2S

   Description          :  Wrapper for FPM

*********************************************************************************************************************************

   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

********************************************************************************************************************************/

module   A2F_m_wrap (
   output wire [63:0]   FPMim,
   output wire [31:0]   FPMo,
   output wire          FPMif,
   output wire          FPMdf,
   output wire          FPMvf,
   output wire          FPMuf,
   output wire          FPMxf,
   input  wire [ 2:0]   fnFPM,
   input  wire [ 1:0]   rmFPM,
   input  wire [31:0]   iaFPM,
   input  wire [31:0]   ibFPM,
   input  wire [31:0]   icFPM
   );

// Intermediates Nets (fpm1 -> fpm2) --------------------------------------------------------------------------------------------
wire           asel;
wire  [7:0]    e2m1,  e2p0,  e2p1;
wire           ezm1,  ezp0,  fpop, inc2, mrz;
wire  [3:0]   mskm1, mskp0, mskp1;
wire [33:0]     nmr;
wire [31:0]      nr;
wire          ovfm1, ovfp0, ovfp1;
wire  [2:0]     ren;
wire           rsgn,  sadj;
wire         unfnm1,unfnp0,unfnp1;

A2F_m1 fpm1 (
   // Outputs
   .imul_x4    (FPMim[63:0]),
   .IF_x4      (FPMif),
   .DF_x4      (FPMdf),
   // Intermediates
   .asel_x4    (  asel),
   .e2m1_x4    (  e2m1[7:0]),
   .e2p0_x4    (  e2p0[7:0]),
   .e2p1_x4    (  e2p1[7:0]),
   .ezm1_x4    (  ezm1),
   .ezp0_x4    (  ezp0),
   .fpop_x4    (  fpop),
   .inc2_x4    (  inc2),
   .mrz_x4     (   mrz),
   .mskm1_x4   ( mskm1[3:0]),
   .mskp0_x4   ( mskp0[3:0]),
   .mskp1_x4   ( mskp1[3:0]),
   .nmr_x4     (   nmr[33:0]),
   .nr_x4      (    nr[31:0]),
   .ovfm1_x4   ( ovfm1),
   .ovfp0_x4   ( ovfp0),
   .ovfp1_x4   ( ovfp1),
   .ren_x4     (   ren[2:0]),
   .rsgn_x4    (  rsgn),
   .sadj_x4    (  sadj),
   .unfnm1_x4  (unfnm1),
   .unfnp0_x4  (unfnp0),
   .unfnp1_x4  (unfnp1),
   // Inputs
   .Fpm_Opcode (fnFPM[ 2:0]),
   .Fpm_Rm     (rmFPM[ 1:0]),
   .Fpm_SrcA   (iaFPM[31:0]),
   .Fpm_SrcB   (ibFPM[31:0]),
   .Fpm_SrcC   (icFPM[31:0])
   );

A2F_m2 fpm2 (
   // Outputs
   .result_x5  (FPMo[31:0]),
   .Fpm_OF_x5  (FPMvf),
   .Fpm_UF_x5  (FPMuf),
   .Fpm_XF_x5  (FPMxf),
   // Intermediates
   .asel_x5    (  asel),
   .e2m1_x4    (  e2m1[7:0]),
   .e2p0_x4    (  e2p0[7:0]),
   .e2p1_x4    (  e2p1[7:0]),
   .ezm1_x4    (  ezm1),
   .ezp0_x4    (  ezp0),
   .fpop_x5    (  fpop),
   .inc2_x5    (  inc2),
   .mrz_x4     (   mrz),
   .mskm1_x4   ( mskm1[3:0]),
   .mskp0_x4   ( mskp0[3:0]),
   .mskp1_x4   ( mskp1[3:0]),
   .nmr_x4     (   nmr[33:0]),
   .nr_x4      (    nr[31:0]),
   .ovfm1_x4   ( ovfm1),
   .ovfp0_x4   ( ovfp0),
   .ovfp1_x4   ( ovfp1),
   .ren_x5     (   ren[2:0]),
   .rsgn_x4    (  rsgn),
   .sadj_x4    (  sadj),
   .unfnm1_x4  (unfnm1),
   .unfnp0_x4  (unfnp0),
   .unfnp1_x4  (unfnp1)
   );

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
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
